// CodeMirror 5 mode for Subtext.
// This is a port of the classification in lsp/src/scanner.rs; see the
// comments there for the reasoning. In short:
//   - `//` starts a comment everywhere, also inside patterns.
//   - `::`, `||` and `=>` only separate the parts of a scope when they are not
//     nested in `( )` or `{ }` inside that scope. A function body starts with a
//     pattern. Patterns end at `=>`; inside them, `( )`, `{ }` and `[ ]` are regex.
//   - `#` or `^...#` starts a register call, `~` is the ghost character.
//   - a word directly followed by `(` is a function call; `def` is a keyword
//     and the following text up to the `{` is the defined function name.
//   - `[ ]` protection: protected code is highlighted as code with the extra
//     style `subtext-protected`. A pattern that starts at protection depth d
//     contains code wherever the depth drops below d, as in
//     `[{ ]#1[ :: ^]~f(^#2)[$ => y }]`, where `~f(^#2)` runs right away.
//
// One difference to the LSP: a call name ends at protection braces here,
// because CodeMirror cannot recolor text it already returned.
(function (CodeMirror) {
    "use strict";

    function top(state) {
        return state.stack[state.stack.length - 1];
    }

    function topSection(state) {
        var frame = top(state);
        return frame && frame.kind === "scope" ? frame.section : null;
    }

    // the protection depth the current pattern started at, or null outside patterns
    function patternDepth(state) {
        var frame = top(state);
        if (!frame) return null;
        if (frame.kind === "group") return frame.depth;
        if (frame.kind === "scope" && frame.section === "pattern") return frame.depth;
        return null;
    }

    function setSection(state, section) {
        var frame = top(state);
        if (frame && frame.kind === "scope") {
            frame.section = section;
            frame.depth = state.depth;
        }
    }

    function tokenPattern(stream, state, depth, prot) {
        var frame = top(state);
        var inScope = frame.kind === "scope";

        if (inScope && stream.match("=>")) {
            setSection(state, "output");
            // a regex class like [^a] must not leak protection out of the pattern
            state.depth = Math.min(state.depth, depth);
            return "subtext-arrow" + prot;
        }
        if (inScope && stream.match("||")) {
            setSection(state, "pattern");
            return "subtext-sep" + prot;
        }

        var ch = stream.next();
        if (ch === "(" || ch === "{") {
            state.stack.push({ kind: "group", depth: depth });
            return "subtext-pattern" + prot;
        }
        if (ch === ")" || ch === "}") {
            if (frame.kind === "group") {
                state.stack.pop();
            } else if (ch === "}") {
                // a scope whose last arm has no =>
                state.stack.pop();
                return "bracket" + prot;
            }
            return "subtext-pattern" + prot;
        }
        // [ ] are regex classes; they only count as protection inside protected text
        if (ch === "[") {
            if (state.depth > 0) state.depth++;
            return "subtext-pattern" + prot;
        }
        if (ch === "]") {
            if (state.depth > 0) {
                state.depth--;
                // leaving the protection: unprotected code inside the pattern follows
                if (state.depth < depth) return "subtext-protection";
            }
            return "subtext-pattern" + prot;
        }

        while (
            !stream.eol() &&
            !/[\s(){}\[\]]/.test(stream.peek()) &&
            !stream.match("//", false) &&
            !(inScope && (stream.match("=>", false) || stream.match("||", false)))
        ) {
            stream.next();
        }
        return "subtext-pattern" + prot;
    }

    // true if the text at the stream position ends a word
    function atBoundary(stream, state) {
        var ch = stream.peek();
        if (/[\s~(){}\[]/.test(ch)) return true;
        if (ch === "]" && state.depth > 0) return true;
        if (stream.match(/^\^*#/, false)) return true;
        if (stream.match("//", false)) return true;
        var section = topSection(state);
        if (section === "input" && stream.match("::", false)) return true;
        if ((section === "output" || section === "pattern") && stream.match("||", false)) return true;
        if (section === "pattern" && stream.match("=>", false)) return true;
        return false;
    }

    function tokenCode(stream, state, prot) {
        // protection braces are not part of a name
        function nameEnds() {
            var ch = stream.peek();
            return /[\s{\[]/.test(ch) || (ch === "]" && state.depth > 0) || stream.match("//", false);
        }
        if (state.defName && !nameEnds()) {
            // every other non-whitespace character up to the { is part of the name
            while (!stream.eol() && !nameEnds()) stream.next();
            return "subtext-defname" + prot;
        }

        // def, possibly with protection braces in it, as in [d]ef
        var def = stream.match(/^d[\[\]]*e[\[\]]*f(?=\s|$)/);
        if (def) {
            for (var j = 1; j < def[0].length; j++) {
                if (def[0][j] === "[") state.depth++;
                if (def[0][j] === "]" && state.depth > 0) state.depth--;
            }
            state.defName = true;
            return "keyword" + (state.depth > 0 ? " subtext-protected" : "");
        }

        if (stream.match(/^\^*#\d*/)) return "subtext-register" + prot;

        var ch = stream.peek();
        var section = topSection(state);

        if (ch === "~") {
            stream.next();
            return "subtext-ghost" + prot;
        }
        if (ch === "[") {
            stream.next();
            state.depth++;
            return "subtext-protection";
        }
        if (ch === "]" && state.depth > 0) {
            stream.next();
            state.depth--;
            return "subtext-protection";
        }
        if (ch === "{") {
            stream.next();
            state.stack.push({
                kind: "scope",
                section: state.defName ? "pattern" : "input",
                depth: state.depth,
            });
            state.defName = false;
            return "bracket" + prot;
        }
        if (ch === "}") {
            stream.next();
            // close the innermost scope, together with anything left open in it
            for (var i = state.stack.length - 1; i >= 0; i--) {
                if (state.stack[i].kind === "scope") {
                    state.stack.length = i;
                    break;
                }
            }
            return "bracket" + prot;
        }
        if (ch === "(") {
            stream.next();
            state.stack.push({ kind: "paren" });
            return "bracket" + prot;
        }
        if (ch === ")") {
            stream.next();
            var frame = top(state);
            if (frame && frame.kind === "paren") state.stack.pop();
            return "bracket" + prot;
        }
        if (section === "input" && stream.match("::")) {
            setSection(state, "pattern");
            return "subtext-sep" + prot;
        }
        if ((section === "output" || section === "pattern") && stream.match("||")) {
            setSection(state, "pattern");
            return "subtext-sep" + prot;
        }
        // only reachable for unprotected code inside a protected pattern
        if (section === "pattern" && stream.match("=>")) {
            setSection(state, "output");
            return "subtext-arrow" + prot;
        }

        // a word: everything up to the next whitespace, ~ or special character
        stream.next();
        while (!stream.eol() && !atBoundary(stream, state)) stream.next();
        if (stream.peek() === "(") return "subtext-call" + prot;
        return null;
    }

    CodeMirror.defineMode("subtext", function () {
        return {
            startState: function () {
                return { stack: [], depth: 0, defName: false };
            },
            copyState: function (state) {
                return {
                    stack: state.stack.map(function (frame) {
                        return Object.assign({}, frame);
                    }),
                    depth: state.depth,
                    defName: state.defName,
                };
            },
            token: function (stream, state) {
                if (stream.eatSpace()) return null;
                if (stream.match("//")) {
                    stream.skipToEnd();
                    return "comment";
                }
                var prot = state.depth > 0 ? " subtext-protected" : "";
                var depth = patternDepth(state);
                if (depth !== null && state.depth >= depth) {
                    return tokenPattern(stream, state, depth, prot);
                }
                return tokenCode(stream, state, prot);
            },
            lineComment: "//",
        };
    });

    CodeMirror.defineMIME("text/x-subtext", "subtext");
})(window.CodeMirror);
