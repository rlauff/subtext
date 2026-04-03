

// summing two digits and potentially a carry indicated by a c

def sum_two_digits {
        00c                                              => 1 
    || (?:01|10)c                                        => 2     
    || (?:02|11|20)c                                     => 3     
    || (?:03|12|21|30)c                                  => 4     
    || (?:04|13|22|31|40)c                               => 5     
    || (?:05|14|23|32|41|50)c                            => 6     
    || (?:06|15|24|33|42|51|60)c                         => 7     
    || (?:07|16|25|34|43|52|61|70)c                      => 8     
    || (?:08|17|26|35|44|53|62|71|80)c                   => 9     
    || (?:09|18|27|36|45|54|63|72|81|90)c                => 0c    
    || (?:19|28|37|46|55|64|73|82|91)c                   => 1c    
    || (?:29|38|47|56|65|74|83|92)c                      => 2c    
    || (?:39|48|57|66|75|84|93)c                         => 3c    
    || (?:49|58|67|76|85|94)c                            => 4c    
    || (?:59|68|77|86|95)c                               => 5c    
    || (?:69|78|87|96)c                                  => 6c    
    || (?:79|88|97)c                                     => 7c    
    || (?:89|98)c                                        => 8c    
    || 99c                                               => 9c    
    || 00                                                => 0     
    || 01|10                                             => 1     
    || 02|11|20                                          => 2     
    || 03|12|21|30                                       => 3     
    || 04|13|22|31|40                                    => 4     
    || 05|14|23|32|41|50                                 => 5     
    || 06|15|24|33|42|51|60                              => 6     
    || 07|16|25|34|43|52|61|70                           => 7     
    || 08|17|26|35|44|53|62|71|80                        => 8     
    || 09|18|27|36|45|54|63|72|81|90                     => 9     
    || 19|28|37|46|55|64|73|82|91                        => 0c    
    || 29|38|47|56|65|74|83|92                           => 1c    
    || 39|48|57|66|75|84|93                              => 2c    
    || 49|58|67|76|85|94                                 => 3c    
    || 59|68|77|86|95                                    => 4c    
    || 69|78|87|96                                       => 5c    
    || 79|88|97                                          => 6c    
    || 89|98                                             => 7c    
    || 99                                                => 8c    
    || (\d?)c                                            => sum_two_digits(1^#1)
    || (\d?)                                             => #1
}


// adds two positive integers
// Matches against Aa+Bb&cP
// a and b are single digits,
// c is a carry (might not be there)
// P is the partial result (might not be there)
// then we add a and be using the above helper function and build the next call to add
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


// takes an integer and computes the the corresponding fibonacci number
// the last match arm is uses as an initializer (matching the passed digit)
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





