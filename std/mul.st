get_file(std/add.st)

def pred_digit {
        9 => 8
    ||  8 => 7
    ||  7 => 6
    ||  6 => 5
    ||  5 => 4
    ||  4 => 3
    ||  3 => 2
    ||  2 => 1
    ||  1 => 0
    ||  * => 0
}

def pred {
        ^(\d*)0$     => pred(^#1)9
    ||  (\d*)(\d)  => #1{pred_digit(^^#2) :: (\d) => #1}
}

def zeroes {
        ^0*1$     => 0
    ||  (\d*)     => zeroes(pred(^^#1))0
}

def mul_with_digit_to_sum {
       0\*(\d*)         => 0 
    ||   1\*(\d*)       => #1
    || (\d)\*(\d*)      => mul_with_digit_to_sum(pred_digit(^^#1)*^#2)+#2
}

def mul_with_digit {
    (\d)\*(\d*)      => sum(mul_with_digit_to_sum(^^#1*^^#2))
}

def mul_to_sum {
        (\d*)\*(\d*)(\d)&(\d+)&(.*) => mul_to_sum(^#1*^#2&{add(^^^#4+1) :: (.*) => #1}&{mul_with_digit(^^^#3*^^^#1) :: (.*) => #1}{zeroes(^^^#4) :: (.*) => #1}+^#5)
    ||  (\d*)\*&(\d*)&(.*)          => #3
    ||  (\d*)\*(\d*)(\d)            => mul_to_sum(^#1*^#2&1&{mul_with_digit(^^^#3*^^^#1) :: (.*) => #1})
}

def mul {
    (\d*)\*(\d*) => sum(mul_to_sum(^^#1*^^#2))
}

print_output(mul_with_digit(2*3))
print_output(mul(113*2312))

