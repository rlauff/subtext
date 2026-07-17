# Subtext

Subtext is a regex-based, mostly functional esoteric programming language; basically, it is a text rewriting system.

This repo contains an interpreter written in Rust. A Wasm web version can be accessed through [this link](https://page.math.tu-berlin.de/~lauff/subtext/index.html).
To run locally, clone this repository and run with 

`cargo run --release -- <path to .stx file>`

The submodule lsp contains an lsp server providing semantic tokens. At the moment, users have to point their prefered editor by hand to the binary at
`subtext/lsp/target/release/lsp`

---

## Documentation

See below for examples, including Fibonacci numbers and Turing machines.

### Scopes
The main component of Subtext is a "scope". In its most basic form, it consists of three parts:

`{ input :: regex pattern => output }`

The pattern is matched against the input, and if a match is found, then the whole scope is replaced by its output. The regex pattern is passed as-is to the [regex crate](https://docs.rs/regex/latest/regex/), see their documentation for specifics on the regex matching.
After a string replacement is performed, the interpreter keeps reading at the beginning of the replacement, i.e. the replacement is read back in instantly. This is done for meta-programming and recursion.

* **Registers:** The pattern may contain unnamed capture groups (surrounded by round braces), which are saved into "registers" once a match is found. The registers can be used via a "register call" `#` (`#3` would be the third capture group). Note that the registers are 1-indexed.
    * *Example:* `{ world, hello :: (.*), (.*) => #2, #1! }` evaluates to `"hello, world!"`.
* **Nesting & Caret Operator:** Note that scopes can be nested. The registers of parent scopes are available using the caret operator `^` (`^^#3` is the third register 2 scopes up).
    * *Example:* `{ world, hello :: (.*), (.*) => { moon, goodbye :: (.*), (.*) => ^#2, ^#1! #2, #1!} }` evaluates to `"hello world! goodbye, moon"`.
* **Evaluation Rules:** The input and output of a scope are evaluated as if they where their own program, until no further changes happen. More specifically, the interpreter applies changes to the input of a scope until no further changes happen, then it tries to match the pattern, saving the new capture groups in registers. After that, the output is fully evaluated and only then is the scope replaced by the resulting output. Note that the pattern is not evaluated at all. It is passed as-is to the regex engine. This is done to prevent nasty collisions with regex symbols and to avoid never-ending character escapes.
* **Whitespace:** The input, pattern, and output are trimmed, meaning that all surrounding (but not internal) whitespace is removed to allow for code formatting.
* **Multiple Match Arms:** Scopes may also contain multiple match arms, separated by `||`. A scope evaluates to the output of the first arm that matches the input. If no arm matches the input, an error is raised.
    * *Example:* `{ foo :: doesnt match => whatever || next pattern => no match || ... => bar || even more arms => unreachable }` evaluates to `"bar"`.

### Functions
Another main component of Subtext are functions. A function is defined using the `def` keyword, followed by a name and function body in curly braces, like this: 
`def foo { ... => bar }` 

The function body is a scope missing the input part. A function is then called with round braces, as in `foo(abc)`, which is equivalent to `{ abc :: ... => bar }`. 
However, functions are not just sugar for scopes, as they enable recursion. 

* **The Ghost Character `~`:** When writing recursive functions, one often runs into the following problem. Say we want to call register 3, and append the digit 1 to the end of its value. We cannot write `#31`, as this calls register 31. Or we want to build the string `10000` by saying `1zeroes(4)`, using a function "zeroes". However, this will try to call the function named "1zeroes". To fix this problem, one can use the "ghost character" `~`. It can be used to separate expressions (as in `#3~1` or `1~zeroes(4)`), but is removed afterwards. 
* **Passing Context:** Note also that a function counts as a scope. To pass a capture group as input, one always needs to add at least one `^`.
    * *Example:* `def swap { (.*)&(.*) => #2#1 } 
        { world, hello :: (.*), (.*) => swap(^#1&^#2) }
        `
### Evaluation Protection

When parsing the current state, anything inside square braces is ignored, but when a scope (or function) returns, then a single layer of square braces is stripped from its fully evaluated output string before the replacement is performed. This enables meta-programming.
 * *Example:* `{ define the function f :: => [d]ef f [{ foo => bar }] }` evaluates to `"def f { foo => bar }"` and is then read back in at the parent scope. If the definition is written normaly inside the scope, then it defines a local function which will not be available in the parent. For more complex examples, see below

### Comments

Any part of the program surrounded by `//` and a newline is a comment and ignored by the interpreter.

---

## Built-in Functions

For IO and debugging, we provide the following built-in functions:

* **`get_file(path)`:** Takes a path, reads the file, and replaces itself by the content of the file.
* **`get_input(prompt)`:** Takes a prompt, prints it to stdout and expects user input via stdin. Then it replaces itself by that input.
* **`print_output(content)`:** Simply prints whatever is passed to it and then replaces itself by the empty string.
* **`debug(...)`:** Enables debug mode for the evaluation of its content. It prints the full history of the evolution of its content through all string replacements done. (Work in progress)

---

## Examples

Here are some examples, ranging from simple to complex.

### Binary Increment
A function which takes a binary number and adds 1 to it: 

```subtext
def to_zeros {
        1(.*) => 0~to_zeros(^#1)
    ||        => 
}

def inc_bin {
        (.*)0(1*) => #1~1~to_zeros(^#2)
    ||  (1+)      => 1~to_zeros(^#1)
    ||            => 1                
}

print_output(inc_bin(1011)) // 1100
```

### Compare
Takes two positive integers, separated by `&`, and compares them. Returns `<`, `>`, or `=`.

```subtext
def compare_digits {
        00      => =
    ||  0[1-9]  => <
    ||  11      => =
    ||  1[2-9]  => <
    ||  22      => =
    ||  2[3-9]  => <
    ||  33      => =
    ||  3[4-9]  => <
    ||  44      => =
    ||  4[5-9]  => <
    ||  55      => =
    ||  5[6-9]  => <
    ||  66      => =
    ||  6[7-9]  => <
    ||  77      => =
    ||  7[8-9]  => <
    ||  88      => =
    ||  8[9]    => <
    ||  99      => =
    ||          => > // we covered all cases where left <= right, so if we get here, left > right
}

def compare {
        (\d*)&(\d*)$                  => compare(^#1&^#2=) // init with = state
    ||  (\d*)(\d)&(\d*)(\d)([<,>,=])    => { 
            compare_digits(^^#2^^#4)  :: (.) => compare(^^#1&^^#3{^^#1 :: ([<,>]) => #1 || = => ^^^#5 })
        }
    ||  &\d                             => <  // left number is empty, right is bigger 
    ||  \d&                             => >  // right number is empty, left is bigger
    ||  &([<,>,=])                      => #1 // numbers have the same length, return the current state
}

print_output(compare(1234&1235)) // <
print_output(compare(1234&1234)) // =
print_output(compare(1235&1234)) // >
print_output(compare(999&1000))  // <
```

### Turing Machine

```subtext
// the state is encoded as <state> <read> => <new_state> <write> <move>
// the states and alphabet can be any non whitespace characters or sequences thereof
// the output HALT of the state table below is reserved for halting
// non-initialized parts of the tape are assumed to be all B (blank)
// the initial state should be put into the START arm of state_table

// the example checks if a binary word is a palindrome. If the final tape is all ..BBBB.., then its a palindrome.

def state_table {
    START => q_start
        || q_start 0 => q_f0 B R
        || q_start 1 => q_f1 B R
        || q_start B => HALT
        || q_f0 0 => q_f0 0 R
        || q_f0 1 => q_f0 1 R
        || q_f0 B => q_c0 B L
        || q_f1 0 => q_f1 0 R
        || q_f1 1 => q_f1 1 R
        || q_f1 B => q_c1 B L
        || q_c0 0 => q_back B L
        || q_c0 1 => HALT
        || q_c0 B => HALT
        || q_c1 1 => q_back B L
        || q_c1 0 => HALT
        || q_c1 B => HALT
        || q_back 0 => q_back 0 L
        || q_back 1 => q_back 1 L
        || q_back B => q_start B R
}

def turing {
    // extract the state, prefix A, symbols around the head a,b and suffix B
    //  state  A   a  >b  B
    (.+) (.*)(.)>(.)(.*) => { 
        state_table(^^#1 ^^#4) :: HALT => ^#2^#3^#4^#5
            ||  (.+) (.) (.) => turing({
//      new_state^   w   ^move

                    // match against the direction to move and whether A or B are empty
                    // if empty, we have to insert a B so that the reading head is never exposed on either side
                    ^^#3&^^^#2&^^^#5 :: L&.+&.* => ^^#1 ^^^#2>^^^#3^^#2^^^#5
                    || L&.*&   => ^^#1 B>^^^#2^^^#3^^#2^^^#5
                    || R&.*&.+ => ^^#1 ^^^#2^^^#3^^#2>^^^#5
                    || R&.*&   => ^^#1 ^^^#2^^^#3^^#2>B
                    }) 
    }
    || (.*) => turing(state_table(START) B>^#1) // initialize the call
}

print_output(turing(1011001))
```

### Fibonacci

```subtext
// summing two digits and potentially a carry indicated by a c
def sum_two_digits {
        00c                                        => 1 
    || (?:01|10)c                                  => 2     
    || (?:02|11|20)c                               => 3     
    || (?:03|12|21|30)c                            => 4     
    || (?:04|13|22|31|40)c                         => 5     
    || (?:05|14|23|32|41|50)c                      => 6     
    || (?:06|15|24|33|42|51|60)c                   => 7     
    || (?:07|16|25|34|43|52|61|70)c                => 8     
    || (?:08|17|26|35|44|53|62|71|80)c             => 9     
    || (?:09|18|27|36|45|54|63|72|81|90)c          => 0c    
    || (?:19|28|37|46|55|64|73|82|91)c             => 1c    
    || (?:29|38|47|56|65|74|83|92)c                => 2c    
    || (?:39|48|57|66|75|84|93)c                   => 3c    
    || (?:49|58|67|76|85|94)c                      => 4c    
    || (?:59|68|77|86|95)c                         => 5c    
    || (?:69|78|87|96)c                            => 6c    
    || (?:79|88|97)c                               => 7c    
    || (?:89|98)c                                  => 8c    
    || 99c                                         => 9c    
    || 00                                          => 0     
    || 01|10                                       => 1     
    || 02|11|20                                    => 2     
    || 03|12|21|30                                 => 3     
    || 04|13|22|31|40                              => 4     
    || 05|14|23|32|41|50                           => 5     
    || 06|15|24|33|42|51|60                        => 6     
    || 07|16|25|34|43|52|61|70                     => 7     
    || 08|17|26|35|44|53|62|71|80                  => 8     
    || 09|18|27|36|45|54|63|72|81|90               => 9     
    || 19|28|37|46|55|64|73|82|91                  => 0c    
    || 29|38|47|56|65|74|83|92                     => 1c    
    || 39|48|57|66|75|84|93                        => 2c    
    || 49|58|67|76|85|94                           => 3c    
    || 59|68|77|86|95                              => 4c    
    || 69|78|87|96                                 => 5c    
    || 79|88|97                                    => 6c    
    || 89|98                                       => 7c    
    || 99                                          => 8c    
    || (\d?)c                                      => sum_two_digits(1^#1)
    || (\d?)                                       => #1
}

// adds two positive integers
// Matches against Aa+Bb&cP
// a and b are single digits,
// c is a carry (might not be there)
// P is the partial result (might not be there)
// then we add a and b using the above helper function and build the next call to add
// the other arms handle the cases where one side is empty

def add {
        // Core addition for digits on both sides
        (\d*)(\d)\+(\d*)(\d)&?(c?)(\d*) => add(^#1+^#3&{ sum_two_digits(^^^#2^^^#4^^^#5) :: (\d)(c?) => #2#1^^#6 })
    
        // Carry: digits remaining on the LEFT (e.g., 1+&c1)
    ||  (\d+)\+&c(\d*)                  => add(^#1+1&^#2)
    
        // Carry: digits remaining on the RIGHT or neither (e.g., +1&c1 or +&c1)
    ||  \+(\d*)&c(\d*)                  => add(^#1+1&^#2)
    
        // Cleanup: digits remaining on the LEFT, no carry (e.g., 1+&3)
    ||  (\d+)\+&?(\d*)                  => #1#2
    
        // Cleanup: digits remaining on the RIGHT or neither (e.g., +1&3)
    ||  \+(\d*)&?(\d*)                  => #1#2
    
        // Fallback
    ||  (\d*)                           => #1
}

// takes positive integers and checks if they are equal by comparing their digits from the right to the left
def are_equal {
    (.*)(.)&(.*)(.)  => { ^#2^#4 :: 00|11|22|33|44|55|66|77|88|99 => are_equal(^^#1&^^#3) || => n }
    ||  .+&             => n
    ||  &.+             => n 
    || &                => y
}

// takes an integer and computes the corresponding fibonacci number
// the last match arm is used as an initializer (matching the passed digit)
// after that, we call with A B n N, where 
// A and B are (Fib(n), Fib(n-1))
// N is how many iterations should be performed in total

def fibonacci {
    (\d+) (\d+) (\d+) (\d+) => 
    {
        are_equal(^^#3&^^#4)    ::  y     => ^#1 
                                ||  n     => fibonacci(add(^^^#1+^^^#2) ^^#1 add(^^^#3+1) ^^#4)
    }
   || (\d+)                   => fibonacci(1 1 1 ^#1)
}

print_output(fibonacci(100))
```

### Variables and arrays

```subtext
// call set_var(name=value) will define a function named get_var_name() that returns value
// the function will be defined in the callers scope 
// if you want to define a function in a higher scope, wrap it in more layers of protection braces []

def set_var { (.+)=(.+) => [def get_var_]#1 [{ => ]#2[}] }

def init_array { 
    (.+) => 
        [d]ef #1_values [{ => }] // init empty array 
        // push redefines the values function with the appended value
        [d]ef #1_push [{ (.+) => [d]ef ]#1[_values [{ => ]] #1[_values()][|#1}}]
}

// displays an array, one element per line
def display_array {
    (.+) => print_output(displaying array ^#1:) display_array_inner(^#1_values())
}

// takes the string of values |a|b|c ... and prints them
def display_array_inner {
        ^\|([^|]+)(.*)$ => print_output(^#1) display_array_inner(^#2)
    ||  => 
}

set_var(x=1)
set_var(y=2)
print_output(x = get_var_x())
print_output(y = get_var_y())

init_array(arr)
arr_push(1)
arr_push(2)
arr_push(3)
arr_push(4)
print_output(array arr values string: arr_values())
display_array(arr)

```
