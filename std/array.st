// defines a new new 
// gets its name as only argument
// this need char escales to work

def new_array {
   (.+) =>  \def array_#1 \{ => ^#1 \} 
            \def push_array_#1 { => \def array_^#1 { (.*) => array_^^#1()&#1 } 
            \def array_contains { (.*) => {...} } // todo
}
