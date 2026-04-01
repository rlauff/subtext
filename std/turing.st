// We try to design a turing machine the alphabet will consist of digits
//The tape is stored in the following format, such that the head points to the cell to the right of "&"
// ... d* d* & d* d* ...
// initially each cell has value 0
// the statetable has to be input via a function of the format 
// The current state of the turing machine is described by a number followed by 
// state ... d* d* & d* d* ...
//the turing machine halts on state 0

//example program, shifting to the left on each step
def table {
        9 \d* => 9 L 8
    ||  8 \d* => 8 L 7
    ||  7 \d* => 7 L 6
    ||  6 \d* => 6 L 5
    ||  5 \d* => 5 L 4
    ||  4 \d* => 4 L 3
    ||  3 \d* => 3 L 2
    ||  2 \d* => 2 R 1
    ||  1 \d* => 1 R 0
}

def turing_step_not_halted {
        (\d+)(.*?) +(\d+) +& +(\d+)(.*) => { table(^^#1  ^^#4) :: (\d*) (.) (\d*) => 
            #3 ^#2  {^#2 :: L => & ^^#3  ^#1 || N => ^^#3  & ^#1 || R => ^^#3  ^#1  &}^#5
        }
    ||   (\d+) +& +(\d+)(.*) => { table(^^#1  ^^#2) :: (\d*) (.) (\d*) => 
            #3  {^#2 :: L => & 0 ^#1 || N => & ^#1 || R => ^#1  &}^#3
        }
    ||   (\d+)(.*?) +(\d+) +& => { table(^^#1  0) :: (\d*) (.) (\d*) => 
            #3 ^#2  {^#2 :: L => & ^^#3  ^#1 || N => ^^#3  & ^#1 || R => ^^#3  ^#1  &}
        }  
}

def print_and_return {
    (.*) => print_output(#1)#1
}

def turing_step {
    (\d+) (.*) => { ^#1 :: 0 => ^#2 || (\d+) => turing_step(turing_step_not_halted(^^^#1  ^^^#2))}
}

def start_turing {
    (\d*) => turing_step(^#1  & 0)
}

def test {
        (\d+)(.*) (\d+) & (\d+)(.*) => #6
    ||  (\d+) & (\d+)(.*) => #6
    ||   (\d+)(.*) (\d+) & => #6
}

print_output(turing_step_not_halted(3 0 0 & 1 1 1 1 1 1 1))

debug(start_turing(9))