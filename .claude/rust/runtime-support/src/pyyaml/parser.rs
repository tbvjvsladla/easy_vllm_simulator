//! `yaml/parser.py` and `yaml/events.py`: tokens to events with the original's state machine.

use super::frames::{frame, nested, occurrence, set_raise};
use super::scanner::{DirectiveValue, Scanner, K};
use super::{marked, Mark, Parsed};
use crate::pystr::PyStr;
use std::collections::BTreeMap;

/// The events of `yaml/events.py`. Fields the SafeLoader never reads (`style`, the collections' `implicit`, an
/// event's end mark) are kept so the port stays line-for-line comparable with the original.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) enum Ev {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Alias {
        anchor: String,
    },
    Scalar {
        anchor: Option<String>,
        tag: Option<String>,
        implicit: (bool, bool),
        value: PyStr,
        style: Option<char>,
    },
    SequenceStart {
        anchor: Option<String>,
        tag: Option<String>,
        implicit: bool,
    },
    SequenceEnd,
    MappingStart {
        anchor: Option<String>,
        tag: Option<String>,
        implicit: bool,
    },
    MappingEnd,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct Event {
    pub ev: Ev,
    pub start: Mark,
    pub end: Mark,
}

#[derive(Clone, Copy, Debug)]
enum State {
    StreamStart,
    ImplicitDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentContent,
    BlockNode,
    BlockSequenceFirstEntry,
    BlockSequenceEntry,
    IndentlessSequenceEntry,
    BlockMappingFirstKey,
    BlockMappingKey,
    BlockMappingValue,
    FlowSequenceFirstEntry,
    FlowSequenceEntry,
    FlowSequenceEntryMappingKey,
    FlowSequenceEntryMappingValue,
    FlowSequenceEntryMappingEnd,
    FlowMappingFirstKey,
    FlowMappingKey,
    FlowMappingValue,
    FlowMappingEmptyValue,
}

pub(crate) struct Parser<'a> {
    pub scanner: Scanner<'a>,
    current: Option<Event>,
    tag_handles: BTreeMap<String, String>,
    states: Vec<State>,
    marks: Vec<Mark>,
    state: Option<State>,
}

fn parser_error(
    raise_line: u32,
    context: Option<&str>,
    context_mark: Option<&Mark>,
    problem: impl Into<String>,
    mark: &Mark,
) -> super::Failure {
    set_raise(raise_line);
    marked("ParserError", context, context_mark, problem, Some(mark))
}

fn repr(text: &str) -> String {
    PyStr::new(text).repr()
}

impl<'a> Parser<'a> {
    pub fn new(scanner: Scanner<'a>) -> Self {
        Parser {
            scanner,
            current: None,
            tag_handles: BTreeMap::new(),
            states: Vec::new(),
            marks: Vec::new(),
            state: Some(State::StreamStart),
        }
    }

    fn advance(&mut self) -> Parsed<()> {
        if self.current.is_none() {
            if let Some(state) = self.state {
                self.current = Some(self.run(state)?);
            }
        }
        Ok(())
    }

    pub fn check_event(&mut self, test: impl Fn(&Ev) -> bool) -> Parsed<bool> {
        let _frame = frame("check_event")?;
        self.advance()?;
        Ok(self.current.as_ref().is_some_and(|event| test(&event.ev)))
    }

    pub fn peek_event(&mut self) -> Parsed<Event> {
        let _frame = frame("peek_event")?;
        self.advance()?;
        Ok(self.current.clone().expect("an event is available"))
    }

    pub fn get_event(&mut self) -> Parsed<Event> {
        let _frame = frame("get_event")?;
        self.advance()?;
        Ok(self.current.take().expect("an event is available"))
    }

    fn run(&mut self, state: State) -> Parsed<Event> {
        match state {
            State::StreamStart => self.parse_stream_start(),
            State::ImplicitDocumentStart => self.parse_implicit_document_start(),
            State::DocumentStart => self.parse_document_start(),
            State::DocumentEnd => self.parse_document_end(),
            State::DocumentContent => self.parse_document_content(),
            State::BlockNode => self.parse_block_node(),
            State::BlockSequenceFirstEntry => {
                let _frame = frame("parse_block_sequence_first_entry")?;
                let token = self.scanner.get_token()?;
                self.marks.push(token.start);
                self.parse_block_sequence_entry()
            }
            State::BlockSequenceEntry => self.parse_block_sequence_entry(),
            State::IndentlessSequenceEntry => self.parse_indentless_sequence_entry(),
            State::BlockMappingFirstKey => {
                let _frame = frame("parse_block_mapping_first_key")?;
                let token = self.scanner.get_token()?;
                self.marks.push(token.start);
                self.parse_block_mapping_key()
            }
            State::BlockMappingKey => self.parse_block_mapping_key(),
            State::BlockMappingValue => self.parse_block_mapping_value(),
            State::FlowSequenceFirstEntry => {
                let _frame = frame("parse_flow_sequence_first_entry")?;
                let token = self.scanner.get_token()?;
                self.marks.push(token.start);
                self.parse_flow_sequence_entry(true)
            }
            State::FlowSequenceEntry => self.parse_flow_sequence_entry(false),
            State::FlowSequenceEntryMappingKey => self.parse_flow_sequence_entry_mapping_key(),
            State::FlowSequenceEntryMappingValue => self.parse_flow_sequence_entry_mapping_value(),
            State::FlowSequenceEntryMappingEnd => self.parse_flow_sequence_entry_mapping_end(),
            State::FlowMappingFirstKey => {
                let _frame = frame("parse_flow_mapping_first_key")?;
                let token = self.scanner.get_token()?;
                self.marks.push(token.start);
                self.parse_flow_mapping_key(true)
            }
            State::FlowMappingKey => self.parse_flow_mapping_key(false),
            State::FlowMappingValue => self.parse_flow_mapping_value(),
            State::FlowMappingEmptyValue => self.parse_flow_mapping_empty_value(),
        }
    }

    fn pop_state(&mut self) -> Option<State> {
        self.states.pop()
    }

    fn parse_stream_start(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_stream_start")?;
        let token = self.scanner.get_token()?;
        self.state = Some(State::ImplicitDocumentStart);
        built(Event {
            ev: Ev::StreamStart,
            start: token.start,
            end: token.end,
        })
    }

    fn parse_implicit_document_start(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_implicit_document_start")?;
        if !self
            .scanner
            .check_token(&[K::Directive, K::DocumentStart, K::StreamEnd])?
        {
            self.tag_handles = default_tags();
            let token = self.scanner.peek_token()?;
            self.states.push(State::DocumentEnd);
            self.state = Some(State::BlockNode);
            built(Event {
                ev: Ev::DocumentStart,
                start: token.start.clone(),
                end: token.start,
            })
        } else {
            self.parse_document_start()
        }
    }

    fn parse_document_start(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_document_start")?;
        while self.scanner.check_token(&[K::DocumentEnd])? {
            self.scanner.get_token()?;
        }
        if !self.scanner.check_token(&[K::StreamEnd])? {
            let token = self.scanner.peek_token()?;
            let start = token.start;
            self.process_directives()?;
            if !self.scanner.check_token(&[K::DocumentStart])? {
                let found = self.scanner.peek_token()?;
                // the original peeks once for the message and once more for the mark
                let again = self.scanner.peek_token()?;
                return Err(parser_error(
                    171,
                    None,
                    None,
                    format!(
                        "expected '<document start>', but found {}",
                        repr(found.kind.id())
                    ),
                    &again.start,
                ));
            }
            let token = self.scanner.get_token()?;
            self.states.push(State::DocumentEnd);
            self.state = Some(State::DocumentContent);
            built(Event {
                ev: Ev::DocumentStart,
                start,
                end: token.end,
            })
        } else {
            let token = self.scanner.get_token()?;
            self.state = None;
            built(Event {
                ev: Ev::StreamEnd,
                start: token.start,
                end: token.end,
            })
        }
    }

    fn parse_document_end(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_document_end")?;
        let token = self.scanner.peek_token()?;
        let (start, mut end) = (token.start.clone(), token.start);
        if self.scanner.check_token(&[K::DocumentEnd])? {
            end = self.scanner.get_token()?.end;
        }
        self.state = Some(State::DocumentStart);
        built(Event {
            ev: Ev::DocumentEnd,
            start,
            end,
        })
    }

    fn parse_document_content(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_document_content")?;
        if self.scanner.check_token(&[
            K::Directive,
            K::DocumentStart,
            K::DocumentEnd,
            K::StreamEnd,
        ])? {
            let mark = self.scanner.peek_token()?.start;
            let event = process_empty_scalar(&mark)?;
            self.state = self.pop_state();
            Ok(event)
        } else {
            self.parse_block_node()
        }
    }

    fn process_directives(&mut self) -> Parsed<()> {
        let _frame = frame("process_directives")?;
        let mut yaml_version: Option<(i64, i64)> = None;
        self.tag_handles = BTreeMap::new();
        while self.scanner.check_token(&[K::Directive])? {
            let token = self.scanner.get_token()?;
            let name = token.text.to_utf8().unwrap_or_default();
            if name == "YAML" {
                if yaml_version.is_some() {
                    return Err(parser_error(
                        224,
                        None,
                        None,
                        "found duplicate YAML directive",
                        &token.start,
                    ));
                }
                if let DirectiveValue::Yaml(major, minor) = token.directive {
                    if major != 1 {
                        return Err(parser_error(
                            228,
                            None,
                            None,
                            "found incompatible YAML document (version 1.* is required)",
                            &token.start,
                        ));
                    }
                    yaml_version = Some((major, minor));
                }
            } else if name == "TAG" {
                if let DirectiveValue::Tag(handle, prefix) = token.directive {
                    if self.tag_handles.contains_key(&handle) {
                        return Err(parser_error(
                            235,
                            None,
                            None,
                            format!("duplicate tag handle {}", repr(&handle)),
                            &token.start,
                        ));
                    }
                    self.tag_handles.insert(handle, prefix);
                }
            }
        }
        for (key, value) in default_tags() {
            self.tag_handles.entry(key).or_insert(value);
        }
        Ok(())
    }

    fn parse_block_node(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_block_node")?;
        self.parse_node(true, false)
    }

    fn parse_flow_node(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_flow_node")?;
        self.parse_node(false, false)
    }

    fn parse_block_node_or_indentless_sequence(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_block_node_or_indentless_sequence")?;
        self.parse_node(true, true)
    }

    fn parse_node(&mut self, block: bool, indentless_sequence: bool) -> Parsed<Event> {
        let _frame = frame("parse_node")?;
        if self.scanner.check_token(&[K::Alias])? {
            let token = self.scanner.get_token()?;
            self.state = self.pop_state();
            return built(Event {
                ev: Ev::Alias {
                    anchor: token.text.to_utf8().unwrap_or_default(),
                },
                start: token.start,
                end: token.end,
            });
        }
        let mut anchor: Option<String> = None;
        let mut tag: Option<(Option<String>, String)> = None;
        let mut start: Option<Mark> = None;
        let mut end: Option<Mark> = None;
        let mut tag_mark: Option<Mark> = None;
        if self.scanner.check_token(&[K::Anchor])? {
            let token = self.scanner.get_token()?;
            start = Some(token.start);
            end = Some(token.end);
            anchor = Some(token.text.to_utf8().unwrap_or_default());
            if self.scanner.check_token(&[K::Tag])? {
                let token = self.scanner.get_token()?;
                tag_mark = Some(token.start);
                end = Some(token.end);
                tag = token.tag;
            }
        } else if self.scanner.check_token(&[K::Tag])? {
            let token = self.scanner.get_token()?;
            start = Some(token.start.clone());
            tag_mark = Some(token.start);
            end = Some(token.end);
            tag = token.tag;
            if self.scanner.check_token(&[K::Anchor])? {
                let token = self.scanner.get_token()?;
                end = Some(token.end);
                anchor = Some(token.text.to_utf8().unwrap_or_default());
            }
        }
        let resolved_tag: Option<String> = match tag {
            None => None,
            Some((Some(handle), suffix)) => {
                let Some(prefix) = self.tag_handles.get(&handle) else {
                    return Err(parser_error(
                        305,
                        Some("while parsing a node"),
                        start.as_ref(),
                        format!("found undefined tag handle {}", repr(&handle)),
                        tag_mark.as_ref().expect("tag mark"),
                    ));
                };
                Some(format!("{prefix}{suffix}"))
            }
            Some((None, suffix)) => Some(suffix),
        };
        let start_mark = match start {
            Some(mark) => mark,
            None => {
                let mark = self.scanner.peek_token()?.start;
                end = Some(mark.clone());
                mark
            }
        };
        let mut end_mark = end.unwrap_or_else(|| start_mark.clone());
        let implicit = resolved_tag.is_none() || resolved_tag.as_deref() == Some("!");
        if indentless_sequence && self.scanner.check_token(&[K::BlockEntry])? {
            end_mark = self.scanner.peek_token()?.end;
            self.state = Some(State::IndentlessSequenceEntry);
            return built(Event {
                ev: Ev::SequenceStart {
                    anchor,
                    tag: resolved_tag,
                    implicit,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        if self.scanner.check_token(&[K::Scalar])? {
            let token = self.scanner.get_token()?;
            end_mark = token.end.clone();
            let implicit = if (token.plain && resolved_tag.is_none())
                || resolved_tag.as_deref() == Some("!")
            {
                (true, false)
            } else if resolved_tag.is_none() {
                (false, true)
            } else {
                (false, false)
            };
            self.state = self.pop_state();
            return built(Event {
                ev: Ev::Scalar {
                    anchor,
                    tag: resolved_tag,
                    implicit,
                    value: token.text,
                    style: token.style,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        if self.scanner.check_token(&[K::FlowSequenceStart])? {
            end_mark = self.scanner.peek_token()?.end;
            self.state = Some(State::FlowSequenceFirstEntry);
            return built(Event {
                ev: Ev::SequenceStart {
                    anchor,
                    tag: resolved_tag,
                    implicit,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        if self.scanner.check_token(&[K::FlowMappingStart])? {
            end_mark = self.scanner.peek_token()?.end;
            self.state = Some(State::FlowMappingFirstKey);
            return built(Event {
                ev: Ev::MappingStart {
                    anchor,
                    tag: resolved_tag,
                    implicit,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        if block && self.scanner.check_token(&[K::BlockSequenceStart])? {
            end_mark = self.scanner.peek_token()?.start;
            self.state = Some(State::BlockSequenceFirstEntry);
            return built(Event {
                ev: Ev::SequenceStart {
                    anchor,
                    tag: resolved_tag,
                    implicit,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        if block && self.scanner.check_token(&[K::BlockMappingStart])? {
            end_mark = self.scanner.peek_token()?.start;
            self.state = Some(State::BlockMappingFirstKey);
            return built(Event {
                ev: Ev::MappingStart {
                    anchor,
                    tag: resolved_tag,
                    implicit,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        if anchor.is_some() || resolved_tag.is_some() {
            self.state = self.pop_state();
            return built(Event {
                ev: Ev::Scalar {
                    anchor,
                    tag: resolved_tag,
                    implicit: (implicit, false),
                    value: PyStr::default(),
                    style: None,
                },
                start: start_mark,
                end: end_mark,
            });
        }
        let node = if block { "block" } else { "flow" };
        let token = self.scanner.peek_token()?;
        Err(parser_error(
            369,
            Some(&format!("while parsing a {node} node")),
            Some(&start_mark),
            format!(
                "expected the node content, but found {}",
                repr(token.kind.id())
            ),
            &token.start,
        ))
    }

    fn parse_block_sequence_entry(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_block_sequence_entry")?;
        if self.scanner.check_token(&[K::BlockEntry])? {
            let token = self.scanner.get_token()?;
            if !self.scanner.check_token(&[K::BlockEntry, K::BlockEnd])? {
                self.states.push(State::BlockSequenceEntry);
                return self.parse_block_node();
            }
            self.state = Some(State::BlockSequenceEntry);
            return process_empty_scalar(&token.end);
        }
        if !self.scanner.check_token(&[K::BlockEnd])? {
            let token = self.scanner.peek_token()?;
            return Err(parser_error(
                392,
                Some("while parsing a block collection"),
                self.marks.last(),
                format!("expected <block end>, but found {}", repr(token.kind.id())),
                &token.start,
            ));
        }
        let token = self.scanner.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        built(Event {
            ev: Ev::SequenceEnd,
            start: token.start,
            end: token.end,
        })
    }

    fn parse_indentless_sequence_entry(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_indentless_sequence_entry")?;
        if self.scanner.check_token(&[K::BlockEntry])? {
            let token = self.scanner.get_token()?;
            if !self
                .scanner
                .check_token(&[K::BlockEntry, K::Key, K::Value, K::BlockEnd])?
            {
                self.states.push(State::IndentlessSequenceEntry);
                return self.parse_block_node();
            }
            self.state = Some(State::IndentlessSequenceEntry);
            return process_empty_scalar(&token.end);
        }
        let token = self.scanner.peek_token()?;
        self.state = self.pop_state();
        built(Event {
            ev: Ev::SequenceEnd,
            start: token.start.clone(),
            end: token.start,
        })
    }

    fn parse_block_mapping_key(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_block_mapping_key")?;
        if self.scanner.check_token(&[K::Key])? {
            let token = self.scanner.get_token()?;
            if !self.scanner.check_token(&[K::Key, K::Value, K::BlockEnd])? {
                self.states.push(State::BlockMappingValue);
                return self.parse_block_node_or_indentless_sequence();
            }
            self.state = Some(State::BlockMappingValue);
            return process_empty_scalar(&token.end);
        }
        if !self.scanner.check_token(&[K::BlockEnd])? {
            let token = self.scanner.peek_token()?;
            return Err(parser_error(
                438,
                Some("while parsing a block mapping"),
                self.marks.last(),
                format!("expected <block end>, but found {}", repr(token.kind.id())),
                &token.start,
            ));
        }
        let token = self.scanner.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        built(Event {
            ev: Ev::MappingEnd,
            start: token.start,
            end: token.end,
        })
    }

    fn parse_block_mapping_value(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_block_mapping_value")?;
        if self.scanner.check_token(&[K::Value])? {
            let token = self.scanner.get_token()?;
            if !self.scanner.check_token(&[K::Key, K::Value, K::BlockEnd])? {
                self.states.push(State::BlockMappingKey);
                return self.parse_block_node_or_indentless_sequence();
            }
            self.state = Some(State::BlockMappingKey);
            return process_empty_scalar(&token.end);
        }
        self.state = Some(State::BlockMappingKey);
        let token = self.scanner.peek_token()?;
        process_empty_scalar(&token.start)
    }

    fn parse_flow_sequence_entry(&mut self, first: bool) -> Parsed<Event> {
        let _frame = frame("parse_flow_sequence_entry")?;
        if !self.scanner.check_token(&[K::FlowSequenceEnd])? {
            if !first {
                if self.scanner.check_token(&[K::FlowEntry])? {
                    self.scanner.get_token()?;
                } else {
                    let token = self.scanner.peek_token()?;
                    return Err(parser_error(
                        483,
                        Some("while parsing a flow sequence"),
                        self.marks.last(),
                        format!("expected ',' or ']', but got {}", repr(token.kind.id())),
                        &token.start,
                    ));
                }
            }
            if self.scanner.check_token(&[K::Key])? {
                let token = self.scanner.peek_token()?;
                self.state = Some(State::FlowSequenceEntryMappingKey);
                return built(Event {
                    ev: Ev::MappingStart {
                        anchor: None,
                        tag: None,
                        implicit: true,
                    },
                    start: token.start,
                    end: token.end,
                });
            } else if !self.scanner.check_token(&[K::FlowSequenceEnd])? {
                self.states.push(State::FlowSequenceEntry);
                return self.parse_flow_node();
            }
        }
        let token = self.scanner.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        built(Event {
            ev: Ev::SequenceEnd,
            start: token.start,
            end: token.end,
        })
    }

    fn parse_flow_sequence_entry_mapping_key(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_flow_sequence_entry_mapping_key")?;
        let token = self.scanner.get_token()?;
        if !self
            .scanner
            .check_token(&[K::Value, K::FlowEntry, K::FlowSequenceEnd])?
        {
            self.states.push(State::FlowSequenceEntryMappingValue);
            self.parse_flow_node()
        } else {
            self.state = Some(State::FlowSequenceEntryMappingValue);
            process_empty_scalar(&token.end)
        }
    }

    fn parse_flow_sequence_entry_mapping_value(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_flow_sequence_entry_mapping_value")?;
        if self.scanner.check_token(&[K::Value])? {
            let token = self.scanner.get_token()?;
            if !self
                .scanner
                .check_token(&[K::FlowEntry, K::FlowSequenceEnd])?
            {
                self.states.push(State::FlowSequenceEntryMappingEnd);
                return self.parse_flow_node();
            }
            self.state = Some(State::FlowSequenceEntryMappingEnd);
            return process_empty_scalar(&token.end);
        }
        self.state = Some(State::FlowSequenceEntryMappingEnd);
        let token = self.scanner.peek_token()?;
        process_empty_scalar(&token.start)
    }

    fn parse_flow_sequence_entry_mapping_end(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_flow_sequence_entry_mapping_end")?;
        self.state = Some(State::FlowSequenceEntry);
        let token = self.scanner.peek_token()?;
        built(Event {
            ev: Ev::MappingEnd,
            start: token.start.clone(),
            end: token.start,
        })
    }

    fn parse_flow_mapping_key(&mut self, first: bool) -> Parsed<Event> {
        let _frame = frame("parse_flow_mapping_key")?;
        if !self.scanner.check_token(&[K::FlowMappingEnd])? {
            if !first {
                if self.scanner.check_token(&[K::FlowEntry])? {
                    self.scanner.get_token()?;
                } else {
                    let token = self.scanner.peek_token()?;
                    return Err(parser_error(
                        549,
                        Some("while parsing a flow mapping"),
                        self.marks.last(),
                        format!("expected ',' or '}}', but got {}", repr(token.kind.id())),
                        &token.start,
                    ));
                }
            }
            if self.scanner.check_token(&[K::Key])? {
                let token = self.scanner.get_token()?;
                if !self
                    .scanner
                    .check_token(&[K::Value, K::FlowEntry, K::FlowMappingEnd])?
                {
                    self.states.push(State::FlowMappingValue);
                    occurrence(0);
                    return self.parse_flow_node();
                }
                self.state = Some(State::FlowMappingValue);
                return process_empty_scalar(&token.end);
            } else if !self.scanner.check_token(&[K::FlowMappingEnd])? {
                self.states.push(State::FlowMappingEmptyValue);
                occurrence(1);
                return self.parse_flow_node();
            }
        }
        let token = self.scanner.get_token()?;
        self.state = self.pop_state();
        self.marks.pop();
        built(Event {
            ev: Ev::MappingEnd,
            start: token.start,
            end: token.end,
        })
    }

    fn parse_flow_mapping_value(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_flow_mapping_value")?;
        if self.scanner.check_token(&[K::Value])? {
            let token = self.scanner.get_token()?;
            if !self
                .scanner
                .check_token(&[K::FlowEntry, K::FlowMappingEnd])?
            {
                self.states.push(State::FlowMappingKey);
                return self.parse_flow_node();
            }
            self.state = Some(State::FlowMappingKey);
            return process_empty_scalar(&token.end);
        }
        self.state = Some(State::FlowMappingKey);
        let token = self.scanner.peek_token()?;
        process_empty_scalar(&token.start)
    }

    fn parse_flow_mapping_empty_value(&mut self) -> Parsed<Event> {
        let _frame = frame("parse_flow_mapping_empty_value")?;
        self.state = Some(State::FlowMappingKey);
        let mark = self.scanner.peek_token()?.start;
        process_empty_scalar(&mark)
    }
}

fn default_tags() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("!".to_string(), "!".to_string()),
        ("!!".to_string(), "tag:yaml.org,2002:".to_string()),
    ])
}

fn process_empty_scalar(mark: &Mark) -> Parsed<Event> {
    let _frame = frame("process_empty_scalar")?;
    built(Event {
        ev: Ev::Scalar {
            anchor: None,
            tag: None,
            implicit: (true, false),
            value: PyStr::default(),
            style: None,
        },
        start: mark.clone(),
        end: mark.clone(),
    })
}

/// The `__init__` of the event class the original instantiates.
fn built(event: Event) -> Parsed<Event> {
    nested(&["__init__"])?;
    Ok(event)
}
