// CodeMirror 5 mode for Subtext.
// This is a direct port of the token classification in lsp/src/scanner.rs:
//   - `::` and `||` (and the `{` opening a function body after `def name`)
//     enter pattern mode; pattern mode ends only at `=>`.
//   - inside a pattern nothing else is special (it is passed verbatim to the
//     regex engine), matching the LSP behaviour.
//   - `//` starts a comment that runs to the end of the line.
//   - `#` or `^` starts a register call and consumes digits, `#` and `^`.
//   - `~` is the ghost character.
//   - a word directly followed by `(` is a function call; `def` is a keyword
//     and the following word is the defined function name.
(function (CodeMirror) {
    "use strict";

    var WORD = /[\p{L}\p{N}_]/u;

    function tokenPattern(stream, state) {
        if (stream.match("=>")) {
            state.inPattern = false;
            return "subtext-arrow";
        }
        if (stream.eatSpace()) return null;
        while (!stream.eol()) {
            if (stream.match("=>", false)) break;
            stream.next();
        }
        return "subtext-pattern";
    }

    function tokenBase(stream, state) {
        if (stream.eatSpace()) return null;

        if (stream.match("//")) {
            stream.skipToEnd();
            return "comment";
        }
        if (stream.match("::") || stream.match("||")) {
            state.inPattern = true;
            return "subtext-sep";
        }
        if (stream.match("=>")) {
            return "subtext-arrow";
        }

        var ch = stream.peek();

        if (ch === "~") {
            stream.next();
            return "subtext-ghost";
        }
        if (ch === "#" || ch === "^") {
            stream.next();
            while (!stream.eol() && /[0-9#^]/.test(stream.peek())) stream.next();
            return "subtext-register";
        }
        if (ch === "{") {
            stream.next();
            if (state.nextCurlyIsBody) {
                state.nextCurlyIsBody = false;
                state.inPattern = true;
            }
            return "bracket";
        }
        if (ch === "}" || ch === "(" || ch === ")") {
            stream.next();
            return "bracket";
        }
        if (WORD.test(ch)) {
            while (!stream.eol() && WORD.test(stream.peek())) stream.next();
            var word = stream.current();
            if (word === "def") {
                state.afterDef = true;
                return "keyword";
            }
            if (state.afterDef) {
                state.afterDef = false;
                state.nextCurlyIsBody = true;
                return "subtext-defname";
            }
            if (stream.peek() === "(") return "subtext-call";
            return null;
        }

        stream.next();
        return null;
    }

    CodeMirror.defineMode("subtext", function () {
        return {
            startState: function () {
                return {
                    inPattern: false,
                    afterDef: false,
                    nextCurlyIsBody: false,
                };
            },
            token: function (stream, state) {
                return state.inPattern
                    ? tokenPattern(stream, state)
                    : tokenBase(stream, state);
            },
            lineComment: "//",
        };
    });

    CodeMirror.defineMIME("text/x-subtext", "subtext");
})(window.CodeMirror);
