get_file(std/add.st)

def are_equal {
    '(.*)(.)&(.*)(.)  => { ^#2^#4 :: 00|11|22|33|44|55|66|77|88|99 => are_equal(^^#1&^^#3) || => n }
    || &                => y
    ||  .*&             => n
    ||  &.*             => n 
}

// takes an integer and computes the the corresponding fibonacci number
// the first match arm is uses as an initializer (matching the passed digit 
// after that, we call with A&B&n&N 
// A and B are (Fib(n), Fib(n-1))
// N is how many iterations should be performed in total


def fibonacci {
    '(\d+) (\d+) (\d+) (\d+) => 
    {
        are_equal(^^#3&^^#4)    ::  y     => ^#1 
                                ||  n     => fibonacci(add(^^^#1+^^^#2) ^^#1  add(^^^#3+1) ^^#4)
    }
   || '(\d+)                   => fibonacci(1 1 1 ^#1)
}

print_output(fibonacci(get_input(Which fibonacci number should be computed?: )))





