def to_zeros {
        1(.*) => 0~to_zeros(^#1)
    ||        => 
}

def inc_bin {
        (.*)0(1*) => #1~1~to_zeros(^#2)
    ||  (1+)      => 1~to_zeros(^#1)
    ||            => 1               
}

print_output(inc_bin(1011))
