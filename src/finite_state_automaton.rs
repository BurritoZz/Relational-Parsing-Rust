use std::collections::{HashSet, HashMap, BTreeSet, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::Write;

use crate::word::{*, self};
use crate::regex::*;

pub type State = u32;

#[derive(Debug)]
enum StateErrors {
    StartNotInStates(State),
    AccNotInStates(State),
    SrcNotInStates(State),
    DestNotInStates(State),
    AtomicStateNotInStates(State),
}


#[derive(Debug)]
pub struct FiniteStateAutomaton {
    states: HashSet<State>,
    accepting_states: HashSet<State>,
    start: State,
    transitions: HashMap<State, HashMap<Symbol, HashSet<(State, Option<Rules>)>>>,
    atomic_to_state: HashMap<(Symbol, Terminal), (State, Option<HashSet<Rules>>)>,
}

impl fmt::Display for FiniteStateAutomaton {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "States: ")?;
        for state in &self.states {
            write!(f, "{} ", state)?;
        }
        write!(f, "\nAccepting states: ")?;
        for state in &self.accepting_states {
            write!(f, "{} ", state)?;
        }
        write!(f, "\nStart state: {}\n", &self.start)?;
        write!(f, "Transitions:\n")?;
        for (state, transition_list) in &self.transitions {
            write!(f, "{}: ", state)?;
            for (symbol, destinations) in transition_list {
                for (state, _) in destinations {
                    write!(f, "|to {} via {}| ", state, symbol)?;
                }
            }
            write!(f, "\n")?;
        }
        write!(f, "Transition to rules:\n")?;
        for (state, transition_list) in &self.transitions {
            for (symbol, destinations) in transition_list {
                for (_, opt_rules) in destinations {
                    if let Some(rules) = opt_rules {
                        write!(f, "{}: {}: ", state, symbol)?;
                        for (nonterminal, rule) in rules {
                            write!(f, "[{} -> ", nonterminal)?;
                            for symbol in rule {
                                write!(f, "{}", symbol)?;
                            }
                            write!(f, "] ")?;
                        }
                        write!(f, "\n")?;
                    }
                }
            }
        }
        write!(f, "Atomic to state:\n")?;
        for ((symbol, terminal), (state, opt_rules)) in &self.atomic_to_state {
            write!(f, "[{}]^({}) {} ", symbol, terminal, state)?;
            if let Some(rule_set) = opt_rules {
                for rules in rule_set {
                    write!(f, "| ")?;
                    for (nonterminal, rule) in rules {
                        write!(f, "[{} -> ", nonterminal)?;
                        for symbol in rule {
                            write!(f, "{}", symbol)?;
                        }
                        write!(f, "] ")?;
                    }
                    write!(f, "|")?;
                }
           }
            write!(f, "\n")?;
        }
        Ok(())
    }
}

impl FiniteStateAutomaton {
    fn new(states: HashSet<State>, accepting_states: HashSet<State>, start: State, transitions: HashMap<State, HashMap<Symbol, HashSet<(State, Option<Rules>)>>>, atomic_to_state: HashMap<(Symbol, Terminal), (State, Option<HashSet<Rules>>)>) -> Result<FiniteStateAutomaton, StateErrors> {
        if !states.contains(&start) {
            return Err(StateErrors::StartNotInStates(start));
        }
        for acc_state in &accepting_states {
            if !states.contains(acc_state) {
                return Err(StateErrors::AccNotInStates(*acc_state));
            }
        }
        for src_state in transitions.keys() {
            if !states.contains(src_state) {
                return Err(StateErrors::SrcNotInStates(*src_state));
            }
        }
        for transition_list in transitions.values() {
            for (symb, destinations) in transition_list {
                for (dest_state, _) in destinations {
                    if !states.contains(dest_state) {
                        return Err(StateErrors::DestNotInStates(*dest_state));
                    }
                }
            }
        }
        for (atomic_state, _) in atomic_to_state.values() {
            if !states.contains(atomic_state) {
                return Err(StateErrors::AtomicStateNotInStates(*atomic_state));
            }
        }
        Ok(FiniteStateAutomaton{states, accepting_states, start, transitions, atomic_to_state})
    }

    pub fn build_fsa(terminals: &HashSet<Terminal>, start_nt: Nonterminal, rules: &HashMap<Nonterminal, HashSet<Word>>) -> FiniteStateAutomaton {
        let start: State = 0;
        let epsilon: State = 1;
        let mut states: HashSet<State> = HashSet::from([start, epsilon]);
        let mut accepting_states: HashSet<State> = HashSet::from([epsilon]);
        let mut transitions: HashMap<State, HashMap<Symbol, HashSet<(State, Option<Rules>)>>> = HashMap::new();
        let mut atomic_to_state: HashMap<(Symbol, Terminal), (State, Option<HashSet<Rules>>)> = HashMap::new();

        transitions.insert(start, HashMap::from([(Symbol::Nonterminal(start_nt), HashSet::from([(epsilon, None)]))]));

        if rules.get(&start_nt).unwrap().contains(&vec![Symbol::Epsilon]) {
            accepting_states.insert(start);
        }

        // add terminal derivations to atomic_to_state
        for terminal in terminals {
            atomic_to_state.insert((Symbol::Terminal(*terminal), *terminal), (epsilon, None));
        }

        let atomic_regex: Regex = Regex::new(terminals, rules);
        let mut regex_to_state: HashMap<VecDeque<WordNode>, (State, State)> = HashMap::new();
        let mut highest_state: State = 1;

        for ((nonterminal, terminal), node) in atomic_regex.regex {
            if let (true, opt_rules) = node.is_e_node_get_rules() {
                atomic_to_state.insert((Symbol::Nonterminal(nonterminal), terminal), (epsilon, opt_rules));
                continue;
            }

            let mut regex_to_state_key: VecDeque<WordNode> = VecDeque::new();
            let mut wordnode_queue: Vec<WordNode> = node.nodes.clone();
            let mut atomic_rules: Option<HashSet<Rules>> = None;

            let mut node_end: State;

            while let Some(wordnode) = wordnode_queue.pop() {

                let prev_key = regex_to_state_key.clone();
                regex_to_state_key.push_front(wordnode.clone());

                let node_start: State;

                if let Some((dest, end)) = regex_to_state.get(&regex_to_state_key) {
                    let (dest, end) = (*dest, *end);
                    if let (true, opt_rule_set) = wordnode.is_e_node_get_rules() {
                        atomic_rules = opt_rule_set;
                        regex_to_state_key.push_front(wordnode.clone());
                        regex_to_state.insert(regex_to_state_key.clone(), (dest, end));
                        continue;
                    } 
                    continue;
                } else {
                    if let Some((dest, end)) = regex_to_state.get(&prev_key) {
                        node_end = *dest;
                        let end = *end;
                        if let (true, opt_rule_set) = wordnode.is_e_node_get_rules() {
                            atomic_rules = opt_rule_set;
                            regex_to_state.insert(regex_to_state_key.clone(), (node_end, end));
                            continue;
                        }
                        highest_state += 1;
                        node_start = highest_state;
                    } else {
                        highest_state += 1;
                        node_start = highest_state;
                        if let (true, opt_rule_set) = wordnode.is_e_node_get_rules() {
                            atomic_rules = opt_rule_set;
                            regex_to_state.insert(regex_to_state_key.clone(), (node_start, node_start));
                            continue;
                        }
                        if wordnode.kleene_star {
                            node_end = node_start;
                        } else {
                            highest_state += 1;
                            node_end = highest_state;
                        }
                    }
                }
                regex_to_state.insert(regex_to_state_key.clone(), (node_start, node_end));

                for (rules, wordnodeword_set) in wordnode.get_by_base_rules() {
                    let mut sub_states: Vec<State> = vec![node_start];
                    let mut opt_penultimate_state: Option<State> = None;
                    for wordnodeword in wordnodeword_set {
                        let mut source: State = node_start;
                        let mut target: State;
                        let mut carried_rules: Rules = Vec::new();

                        for index in 0..wordnodeword.len() {
                            if index == sub_states.len() - 1 {
                                if index == wordnodeword.len() - 1 {
                                    sub_states.push(node_end);
                                } else {
                                    highest_state += 1;
                                    sub_states.push(highest_state);
                                }
                            }

                            if let WordNodeSymbol::Rules(word_rules) = &wordnodeword[index] {
                                carried_rules = [word_rules.clone(), carried_rules].concat();
                                target = sub_states[index+1];
                                if target == node_end {
                                    let entry = transitions.entry(source).or_default().entry(Symbol::Epsilon).or_default();
                                    
                                    entry.insert((target, Some([rules.clone(), carried_rules].concat())));
                                    //if source == node_start {
                                    //    entry.insert((target, Some([carried_rules, rules.clone()].concat())));
                                    //} else {
                                    //    entry.insert((target, Some(carried_rules)));
                                    //}
                                    source = sub_states[index+1];
                                    carried_rules = Vec::new();
                                }
                            } else {
                                let entry = transitions.entry(source).or_default().entry(
                                    match wordnodeword[index] {
                                        WordNodeSymbol::Nonterminal(nt) => Symbol::Nonterminal(nt),
                                        WordNodeSymbol::Terminal(t) => Symbol::Terminal(t),
                                        WordNodeSymbol::Epsilon => Symbol::Epsilon,
                                        _ => continue,
                                    }
                                ).or_default();

                                target = sub_states[index+1];
                                if target == node_end {
                                    if let WordNodeSymbol::Nonterminal(_) = wordnodeword[index] {
                                        let penultimate_state;
                                        if let Some(state) = opt_penultimate_state {
                                            penultimate_state = state;
                                        } else {
                                            highest_state += 1;
                                            penultimate_state = highest_state;
                                            opt_penultimate_state = Some(penultimate_state);
                                        }
                                        entry.insert((penultimate_state, Some(carried_rules)));
                                        transitions.entry(penultimate_state).or_default().entry(Symbol::Epsilon).or_default().insert((target, Some(rules.clone())));
                                    } else {
                                        entry.insert((target, Some([rules.clone(), carried_rules].concat())));
                                    }
                                    carried_rules = Vec::new();
                                }
                                //if source == node_start {
                                //    entry.insert((target, Some([carried_rules, rules.clone()].concat())));
                                //    carried_rules = Vec::new();
                                //}
                                else if carried_rules.len() > 0 {
                                    entry.insert((target, Some(carried_rules)));
                                    carried_rules = Vec::new();
                                } else {
                                    entry.insert((target, None));
                                }
                                source = sub_states[index+1];
                            }
                        }
                    }
                    states.extend(sub_states);
                    if let Some(state) = opt_penultimate_state {
                        states.insert(state);
                    }
                }


            }
            accepting_states.insert(regex_to_state.get(&regex_to_state_key).unwrap().1);
            atomic_to_state.insert((Symbol::Nonterminal(nonterminal), terminal), (regex_to_state.get(&regex_to_state_key).unwrap().0, atomic_rules));
        }

        FiniteStateAutomaton{states, accepting_states, start, transitions, atomic_to_state}
    }

    pub fn to_dot(&self, filename: &str) -> std::io::Result<()> {
        let mut file = File::create(format!("{}.dot", filename))?;
        write!(file, "digraph G {{\n")?;
        //write!(file, "{} [ label=\"start\" ]\n", &self.start)?;
        let mut state_to_shape: HashMap<State, &str> = HashMap::new();
        for state in &self.states {
            if self.accepting_states.contains(state) {
                state_to_shape.insert(*state, "doublecircle");
            } else {
                state_to_shape.insert(*state, "circle");
            }
        }
        for state in &self.states {
            write!(file, "{} [ shape={} ]\n", state, state_to_shape.get(state).unwrap())?;
        }
        for ((symbol, terminal), (state, opt_rule_set)) in &self.atomic_to_state {
            match symbol {
                Symbol::Nonterminal(nonterm) => {
                    write!(file, "\"[{}]^({})\" [ shape=rectangle ]\n\"[{}]^({})\" -> {}", nonterm, terminal, nonterm, terminal, state)?;
                    if let Some(rule_set) = opt_rule_set {
                        write!(file, "[ label=\"")?;
                        for rules in rule_set {
                            write!(file, "(")?;
                            for rule in rules {
                                write!(file, "[{} -> ", &rule.0)?;
                                for symbol in &rule.1 {
                                    write!(file, "{}", symbol)?;
                                }
                                write!(file, "]")?;
                            }
                            write!(file, ")")?;
                        }
                        write!(file, "\" ]")?;
                    }
                    write!(file, "\n")?;
                },
                Symbol::Terminal(term) => write!(file, "\"[{}]^({})\" [ shape=rectangle ]\n\"[{}]^({})\" -> {}\n", term, terminal, term, terminal, state)?,
                _ => {},
            }
        }
        for (source, transition_list) in &self.transitions {
            for (symbol, destinations) in transition_list {
                for (dest, opt_rules) in destinations {
                    match symbol {
                        Symbol::Epsilon => write!(file, "{} -> {} [ label=\"e ", source, dest)?,
                        Symbol::Nonterminal(nonterminal) => write!(file, "{} -> {} [ label=\"{} ", source, dest, nonterminal)?,
                        Symbol::Terminal(terminal) => write!(file, "{} -> {} [ label=\"{} ", source, dest, terminal)?,
                    }
                    if let Some(rules) = opt_rules {
                        for rule in rules {
                            write!(file, "[{} -> ", &rule.0)?;
                            for symbol in &rule.1 {
                                write!(file, "{}", symbol)?;
                            }
                            write!(file, "] ")?;
                        }
                    }
                    write!(file, "\" ]\n")?;
                }
            }
        }
        write!(file, "}}")
    }

    pub fn simulate(&self, curr_state: &State, symbol: Symbol) -> Option<HashSet<(&State, &Option<Rules>, bool)>> {
        self.transitions.get(&curr_state)?
            .get(&symbol)
            .map(|destinations| {
                let mut res: HashSet<(&State, &Option<Rules>, bool)> = HashSet::new();
                for (dest, opt_rules) in destinations {
                    res.insert((dest, opt_rules, self.is_accepting(dest)));
                }
                res
            })
    }

    pub fn is_accepting(&self, curr_state: &State) -> bool {
        self.accepting_states.contains(curr_state)
    }

    pub fn get_start(&self) -> (State, bool) {
        (self.start, self.is_accepting(&self.start))
    }

    pub fn get_atomic(&self, symbol: Symbol, terminal: Terminal) -> Option<(&State, &Option<HashSet<Rules>>, bool)> {
        self.atomic_to_state.get(&(symbol, terminal))
            .map(|(dest, opt_rules_set)| (dest, opt_rules_set, self.is_accepting(dest)))
    }

    pub fn has_transition(&self, curr_state: &State) -> bool {
        self.transitions.get(curr_state)
            .map_or(false, |trans_list| !trans_list.is_empty())
    }

}