#[derive(Clone)]
struct CharNode {
    pub c: char,
    next: Option<usize>, // index into arena of a LinkedChars object
}

pub struct LinkedChars {
    arena: Vec<CharNode>, // the node at index 0 (the root) is not considered part of the content
}

impl CharNode {
    fn new(c: char) -> Self {
        CharNode { c, next: None }
    }
}

impl LinkedChars {
    pub fn from_iter(chars: impl IntoIterator<Item = char>) -> Self {
        let mut char_nodes = vec![CharNode {
            c: ' ',
            next: Some(1),
        }];
        char_nodes.extend(chars.into_iter().enumerate().map(|(i, c)| CharNode {
            c,
            next: Some(i + 2),
        }));
        char_nodes.last_mut().unwrap().next = None;
        LinkedChars { arena: char_nodes }
    }

    pub fn get(&self, index: usize) -> &CharNode {
        &self.arena[index]
    }

    pub fn get_mut(&mut self, index: usize) -> &mut CharNode {
        &mut self.arena[index]
    }

    fn is_empty(&self) -> bool {
        self.arena.len() == 1
    }
    // links the node at start_idx to the next of end_idx
    // this removes all nodes in between, excluding the one at start_idx
    // but including the one at end_idx
    fn remove_between(&mut self, start_idx: usize, end_idx: usize) {
        if start_idx >= self.arena.len() || end_idx >= self.arena.len() {
            panic!("Tried to remove out of range") // TODO: add proper error types
        }
        // note that this is correct even if there is the node at end_idx is the last one
        // in this case we set the .next of the start node to None, which is what we want
        self.get_mut(start_idx).next = self.get(end_idx).next;
    }

    fn replace_between(&mut self, start_idx: usize, end_idx: usize, linked_chars: LinkedChars) {
        // if the passed chars are empty, then we have nothing to do
        if linked_chars.is_empty() {
            return;
        }
        // init this to 0 so we will add the .next of the root node in the next step
        let node_to_add_idx = 0;
        // pretend we just added the node at start_index
        let last_node_added_idx = start_idx;
        // if there is one, pick the next node from linked_chars
        while let Some(node_to_add_idx) = linked_chars.get(node_to_add_idx).next {
            self.arena.push(linked_chars.get(node_to_add_idx).clone());
            // the .node just added should be the .next of the last node added
            // the index of the just pushed node is len-1
            self.get_mut(last_node_added_idx).next = Some(self.arena.len() - 1);
        }
        self.arena.last_mut().unwrap().next = Some(end_idx);
    }
}
