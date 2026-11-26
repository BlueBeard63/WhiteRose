.pragma library
// Selection/cursor-aware operand editing, separate from QML for tests.
// Functions wrap a selected expression, or the operand just before the cursor.

function operandRange(text, cursor, depth) {
    if ((depth || 0) > 64) return [cursor, cursor];
    var end = Math.max(0, Math.min(cursor, text.length));
    while (end > 0 && /\s/.test(text[end - 1])) end--;
    var start = end, scan = end;
    while (scan > 0 && text[scan - 1] === "!") scan--;
    if (scan > 0 && text[scan - 1] === ")") {
        var depth = 1;
        start = scan - 1;
        while (start > 0 && depth > 0) {
            start--;
            if (text[start] === ")") depth++;
            else if (text[start] === "(") depth--;
        }
        if (depth !== 0) return [cursor, cursor];
        while (start > 0 && /[a-z0-9]/i.test(text[start - 1])) start--;
    } else {
        var number = /(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.exec(text.slice(0, scan));
        var constant = /(?:pi|tau|e|π|τ)$/i.exec(text.slice(0, scan));
        if (number) start = scan - number[0].length;
        else if (constant) start = scan - constant[0].length;
        else return [cursor, cursor];
    }
    // A minus after an operator (or at the start) belongs to this operand.
    var prefix = text.slice(0, start);
    var negative = /(^|[+*/%^(−-])\s*-\s*$/.exec(prefix);
    if (negative) start = prefix.lastIndexOf("-");
    // A power belongs to its base, including a signed exponent. CE after
    // x² removes the whole squared operand, not just the exponent's 2.
    var before = text.slice(0, start).replace(/\s+$/, "");
    if (before.endsWith("^")) {
        var base = operandRange(text, before.length - 1, (depth || 0) + 1);
        if (base[0] < before.length - 1) start = base[0];
    }
    return [start, end];
}

function transform(text, cursor, selectionStart, selectionEnd, operation) {
    var range = selectionEnd > selectionStart
        ? [selectionStart, selectionEnd] : operandRange(text, cursor);
    var value = text.slice(range[0], range[1]) || "0";
    var replacement;
    switch (operation) {
    case "square": replacement = "(" + value + ")^2"; break;
    case "reciprocal": replacement = "1/(" + value + ")"; break;
    case "negate": replacement = "-(" + value + ")"; break;
    case "factorial": replacement = "(" + value + ")!"; break;
    case "clear": replacement = ""; break;
    default: replacement = operation + "(" + value + ")"; break;
    }
    return {
        text: text.slice(0, range[0]) + replacement + text.slice(range[1]),
        cursor: range[0] + replacement.length
    };
}

function keys(scientific) {
    return scientific ? [
        "(", ")", "%", "CE", "C", "⌫",
        "xʸ", "n!", "1/x", "x²", "√x", "÷",
        "sin", "asin", "7", "8", "9", "×",
        "cos", "acos", "4", "5", "6", "−",
        "tan", "atan", "1", "2", "3", "+",
        "ln", "log", "±", "0", ".", "="
    ] : [
        "%", "CE", "C", "⌫", "1/x", "x²", "√x", "÷",
        "7", "8", "9", "×", "4", "5", "6", "−",
        "1", "2", "3", "+", "±", "0", ".", "="
    ];
}
