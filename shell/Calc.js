.pragma library
// Shared by the launcher and Calculator (embedded by calculator.qrc).
// A bounded expression parser, never eval. evaluate() preserves the launcher's
// radians and hides bare numbers; calculate() also accepts them for the app.

var FUNCS = {
    sqrt: Math.sqrt, cbrt: Math.cbrt, abs: Math.abs, round: Math.round,
    floor: Math.floor, ceil: Math.ceil, trunc: Math.trunc, sign: Math.sign,
    sin: Math.sin, cos: Math.cos, tan: Math.tan,
    asin: Math.asin, acos: Math.acos, atan: Math.atan,
    sinh: Math.sinh, cosh: Math.cosh, tanh: Math.tanh,
    asinh: Math.asinh, acosh: Math.acosh, atanh: Math.atanh,
    ln: Math.log, log: Math.log10, log10: Math.log10, log2: Math.log2,
    exp: Math.exp, percent: function(x) { return x / 100; },
    fact: factorial
};
var CONSTS = { pi: Math.PI, e: Math.E, tau: 2 * Math.PI };
var PREC = { "+": 10, "-": 10, "*": 20, "/": 20, "%": 20, "^": 30 };
var MAX_INPUT = 4096;
var MAX_TOKENS = 1024;
var MAX_DEPTH = 64;

function owns(object, key) { return Object.prototype.hasOwnProperty.call(object, key); }

function factorial(x) {
    if (!Number.isInteger(x) || x < 0) throw new Error("Factorial needs a non-negative whole number");
    if (x > 170) throw new Error("Result is out of range");
    var result = 1;
    for (var n = 2; n <= x; n++) result *= n;
    return result;
}

function finite(value) {
    if (Number.isNaN(value)) throw new Error("This calculation has no real result");
    if (!Number.isFinite(value)) throw new Error("Result is out of range");
    return value;
}

function tokenize(input) {
    var tokens = [], i = 0;
    while (i < input.length) {
        if (tokens.length >= MAX_TOKENS) throw new Error("Expression is too long");
        var c = input[i];
        if (/\s/.test(c)) { i++; continue; }
        var number = /^(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?/i.exec(input.slice(i));
        if (number) {
            tokens.push({ type: "number", value: finite(Number(number[0])) });
            i += number[0].length;
            continue;
        }
        if (c === "π" || c === "τ") {
            tokens.push({ type: "number", value: c === "π" ? Math.PI : 2 * Math.PI, named: true });
            i++;
            continue;
        }
        var word = /^[a-z][a-z0-9]*/i.exec(input.slice(i));
        if (word) {
            var name = word[0].toLowerCase();
            if (owns(CONSTS, name)) tokens.push({ type: "number", value: CONSTS[name], named: true });
            else if (owns(FUNCS, name)) tokens.push({ type: "function", value: name });
            else throw new Error("Unknown function or constant");
            i += word[0].length;
            continue;
        }
        if (c === "×") c = "*";
        else if (c === "÷") c = "/";
        else if (c === "−") c = "-";
        if ("+-*/%^()!".indexOf(c) < 0) throw new Error("Not a calculation");
        tokens.push({ type: "operator", value: c });
        i++;
    }
    return tokens;
}

function callFunction(name, value, degrees) {
    if (name === "sin" || name === "cos" || name === "tan") {
        var radians = degrees ? value * Math.PI / 180 : value;
        if (name === "tan" && Math.abs(Math.cos(radians)) < 1e-14)
            throw new Error("Tangent is undefined at this angle");
        return finite(FUNCS[name](radians));
    }
    var answer = finite(FUNCS[name](value));
    if (degrees && (name === "asin" || name === "acos" || name === "atan")) answer *= 180 / Math.PI;
    return finite(answer);
}

// Pratt parser. Unary signs bind less tightly than powers, but more tightly
// than multiplication: -2^2 = -4, 2^-1 = 0.5, and 2^3^2 = 512.
function parse(tokens, degrees) {
    var at = 0;
    function expression(minimum, depth) {
        if (depth > MAX_DEPTH) throw new Error("Expression is nested too deeply");
        var token = tokens[at++], left;
        if (!token) throw new Error("Finish the expression");
        if (token.type === "number") left = token.value;
        else if (token.value === "-" || token.value === "+") {
            left = expression(25, depth + 1);
            if (token.value === "-") left = -left;
        } else if (token.value === "(") {
            left = expression(0, depth + 1);
            if (!tokens[at] || tokens[at++].value !== ")") throw new Error("Close the parentheses");
        } else if (token.type === "function") {
            if (!tokens[at] || tokens[at++].value !== "(") throw new Error("Use parentheses after a function");
            var argument = expression(0, depth + 1);
            if (!tokens[at] || tokens[at++].value !== ")") throw new Error("Close the parentheses");
            left = callFunction(token.value, argument, degrees);
        } else throw new Error("Finish the expression");
        while (at < tokens.length) {
            var op = tokens[at].value;
            if (op === "!") {
                if (40 < minimum) break;
                at++;
                left = factorial(left);
                continue;
            }
            if (!owns(PREC, op) || PREC[op] < minimum) break;
            at++;
            var right = expression(PREC[op] + (op === "^" ? 0 : 1), depth + 1);
            if ((op === "/" || op === "%") && right === 0) throw new Error("Cannot divide by zero");
            switch (op) {
            case "+": left += right; break;
            case "-": left -= right; break;
            case "*": left *= right; break;
            case "/": left /= right; break;
            case "%": left %= right; break; // Keep the launcher's remainder operator.
            case "^": left = Math.pow(left, right); break;
            }
            finite(left);
        }
        return finite(left);
    }
    var value = expression(0, 0);
    if (at !== tokens.length) throw new Error("Check the expression");
    return value;
}

function format(value) {
    if (!Number.isFinite(value)) return null;
    if (value === 0) return "0"; // Includes negative zero.
    if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value);
    var parts = value.toPrecision(12).split("e");
    if (parts[0].indexOf(".") >= 0) parts[0] = parts[0].replace(/0+$/, "").replace(/\.$/, "");
    return parts[0] + (parts.length === 2 ? "e" + parts[1] : "");
}

// {ok, result, value} or {ok: false, error}. Without options, angles are radians.
function calculate(input, options) {
    try {
        if (typeof input !== "string" || input.length > MAX_INPUT) throw new Error("Expression is too long");
        var source = input.trim().replace(/=$/, "").trim();
        if (!source) throw new Error("Enter a calculation");
        var tokens = tokenize(source);
        if (options && options.requireCalculation && !tokens.some(function(t) {
            return t.type === "function" || t.named || (t.type === "operator" && t.value !== "(" && t.value !== ")");
        })) throw new Error("Enter a calculation");
        var value = parse(tokens, options && options.angle === "deg");
        return { ok: true, result: format(value), value: value };
    } catch (e) {
        return { ok: false, error: e.message || "Check the expression" };
    }
}

// Backwards-compatible launcher API: a result string or null.
function evaluate(input) {
    var result = calculate(input, { requireCalculation: true });
    return result.ok ? result.result : null;
}
