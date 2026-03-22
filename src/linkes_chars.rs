#[derive(Clone)]
pub struct CharNode {
    pub c: char,
    pub next: Option<usize>, // index into arena of a LinkedChars object
}

pub struct LinkedChars {
    arena: Vec<CharNode>, // the node at index 0 (the root) is not considered part of the content
}

pub struct LinkedCharsIter<'a> {
    linked_chars: &'a LinkedChars,
    idx: usize,
}

impl<'a> Iterator for LinkedCharsIter<'a> {
    type Item = (usize, &'a CharNode);

    fn next(&mut self) -> Option<Self::Item> {
        let old_idx = self.idx;
        if let Some(new_idx) = self.linked_chars.get(self.idx).next {
            self.idx = new_idx;
            Some((old_idx, self.linked_chars.get(new_idx)))
        } else {
            None
        }
    }
}

pub struct LinkedCharsOwnedIter {
    linked_chars: LinkedChars,
    idx: usize,
}

impl Iterator for LinkedCharsOwnedIter {
    type Item = (usize, CharNode);

    fn next(&mut self) -> Option<Self::Item> {
        let old_idx = self.idx;
        if let Some(new_idx) = self.linked_chars.arena[self.idx].next {
            self.idx = new_idx;
            let dummy_node = CharNode {
                c: '\0',
                next: None,
            };
            let owned_node = std::mem::replace(&mut self.linked_chars.arena[new_idx], dummy_node);

            Some((old_idx, owned_node))
        } else {
            None
        }
    }
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
        // pretend we just added the node at start_index
        let last_node_added_idx = start_idx;
        for (_, new_node) in linked_chars.into_enumerator_with_start(0) {
            self.arena.push(new_node);
            // the .node just added should be the .next of the last node added
            // the index of the just pushed node is len-1
            self.get_mut(last_node_added_idx).next = Some(self.arena.len() - 1);
        }
        self.arena.last_mut().unwrap().next = Some(end_idx);
    }

    // returns the subinterval between start_idx and end_idx (non-inclusive on both ends)
    pub fn interval_to_string(&self, start_idx: usize, end_idx: usize) -> String {
        let mut buffer = Vec::new();
        for (i, node) in self.enumerate_with_start(start_idx) {
            if i == end_idx {
                return buffer.into_iter().collect();
            };
            buffer.push(node.c);
        }
        panic!("end_idx was never found");
    }

    pub fn enumerate_with_start(&self, start: usize) -> impl Iterator<Item = (usize, &CharNode)> {
        LinkedCharsIter {
            linked_chars: self,
            idx: start,
        }
    }

    pub fn into_enumerator_with_start(
        self,
        start: usize,
    ) -> impl IntoIterator<Item = (usize, CharNode)> {
        LinkedCharsOwnedIter {
            linked_chars: self,
            idx: start,
        }
    }
}
