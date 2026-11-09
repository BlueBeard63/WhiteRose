.pragma library
// Launcher calculator: a small expression parser (no eval), so text typed
// into the search field can never run code.
//
// Supports + - * / % ^, parentheses, unary minus, decimals and exponents
// (1.5e3), constants pi and e, and sqrt abs round floor ceil sin cos tan
// ln log. Returns null for anything that isn't a calculation (including a
// bare number, which isn't worth "calculating").

var FUNCS = {
    sqrt: Math.sqrt, abs: Math.abs, round: Math.round, floor: Math.floor, ceil: Math.ceil,
    sin: Math.sin, cos: Math.cos, tan: Math.tan, ln: Math.log, log: Math.log10
};
var CONSTS = { pi: Math.PI, e: Math.E };
// Unary minus sits between * and ^, so -2^2 is -(2^2) = -4, as in maths.
var PREC = { "+": 1, "-": 1, "*": 2, "/": 2, "%": 2, "neg": 2.5, "^": 3 };

function tokenize(s) {
    var out = [], i = 0;
    while (i < s.length) {
        var c = s[i];
        if (c === " " || c === "\t") { i++; continue; }
        var num = /^(\d+\.?\d*|\.\d+)(e[+-]?\d+)?/i.exec(s.slice(i));
        if (num) { out.push({ t: "num", v: parseFloat(num[0]) }); i += num[0].length; continue; }
        var word = /^[a-z]+/i.exec(s.slice(i));
        if (word) {
            var w = word[0].toLowerCase();
            if (w in CONSTS) out.push({ t: "num", v: CONSTS[w], c: true });
            else if (w in FUNCS) out.push({ t: "fn", v: w });
            else return null;
            i += word[0].length; continue;
        }
        if ("+-*/%^()".indexOf(c) >= 0 || c === "×" || c === "÷") {
            out.push({ t: "op", v: c === "×" ? "*" : c === "÷" ? "/" : c }); i++; continue;
        }
        return null;
    }
    return out;
}

// Shunting-yard to RPN.
function toRpn(tokens) {
    var out = [], ops = [], prev = null;
    for (var i = 0; i < tokens.length; i++) {
        var tk = tokens[i];
        if (tk.t === "num") out.push(tk);
        else if (tk.t === "fn") ops.push(tk);
        else if (tk.v === "(") ops.push(tk);
        else if (tk.v === ")") {
            while (ops.length && ops[ops.length - 1].v !== "(") out.push(ops.pop());
            if (!ops.length) return null;
            ops.pop();
            if (ops.length && ops[ops.length - 1].t === "fn") out.push(ops.pop());
        } else {
            var op = tk.v;
            var unary = op === "-" && (prev === null || (prev.t === "op" && prev.v !== ")"));
            if (op === "+" && (prev === null || (prev.t === "op" && prev.v !== ")"))) { prev = tk; continue; }
            if (unary) {
                // A prefix operator has no left operand: push it without popping.
                ops.push({ t: "op", v: "neg" });
                prev = tk;
                continue;
            }
            while (ops.length) {
                var top = ops[ops.length - 1];
                if (top.t === "fn") { out.push(ops.pop()); continue; }
                if (top.v === "(") break;
                var rightAssoc = op === "^" || op === "neg";
                if (PREC[top.v] > PREC[op] || (PREC[top.v] === PREC[op] && !rightAssoc)) out.push(ops.pop());
                else break;
            }
            ops.push({ t: "op", v: op });
        }
        prev = tk;
    }
    while (ops.length) {
        var o = ops.pop();
        if (o.v === "(") return null;
        out.push(o);
    }
    return out;
}

function run(rpn) {
    var st = [];
    for (var i = 0; i < rpn.length; i++) {
        var tk = rpn[i];
        if (tk.t === "num") { st.push(tk.v); continue; }
        if (tk.t === "fn") { if (!st.length) return null; st.push(FUNCS[tk.v](st.pop())); continue; }
        if (tk.v === "neg") { if (!st.length) return null; st.push(-st.pop()); continue; }
        if (st.length < 2) return null;
        var b = st.pop(), a = st.pop();
        switch (tk.v) {
        case "+": st.push(a + b); break;
        case "-": st.push(a - b); break;
        case "*": st.push(a * b); break;
        case "/": st.push(a / b); break;
        case "%": st.push(a % b); break;
        case "^": st.push(Math.pow(a, b)); break;
        }
    }
    return st.length === 1 ? st[0] : null;
}

function format(v) {
    if (!isFinite(v)) return null;
    if (Number.isInteger(v) && Math.abs(v) < 1e15) return String(v);
    var s = v.toPrecision(12);
    if (s.indexOf("e") < 0 && s.indexOf(".") >= 0) s = s.replace(/0+$/, "").replace(/\.$/, "");
    return s;
}

// Returns the formatted result, or null.
function evaluate(input) {
    var s = (input || "").trim().replace(/=$/, "");
    if (!s) return null;
    var tokens = tokenize(s);
    if (!tokens || !tokens.length) return null;
    // Must be a calculation: an operator, a function, or a named constant.
    if (!tokens.some(function (t) { return t.t === "fn" || t.c || (t.t === "op" && t.v !== "(" && t.v !== ")"); })) return null;
    var rpn = toRpn(tokens);
    if (!rpn) return null;
    var v = run(rpn);
    return v === null ? null : format(v);
}
