use std::rc::Rc;
use std::cell::RefCell;
use std::slice::Items;
use std::os;

use collections::hashmap::HashMap;

use action::Action;
use action::{ParseResult, Parsed, Exit, Error};
use action::TypedAction;
use action::{Flag, Single, Many};
use action::IArgAction;

mod action;

enum ArgumentKind {
    Positional,
    ShortOption,
    LongOption,
}

impl ArgumentKind {
    fn check(name: &str) -> ArgumentKind {
        let mut iter = name.chars();
        let char1 = iter.next();
        let char2 = iter.next();
        return match char1 {
            Some('-') => {
                match char2 {
                    Some('-') => { LongOption } // --opts
                    Some(_) => { ShortOption }  // -opts
                    None => { Positional }  // single dash
                }
            }
            Some(_) | None => { Positional }
        }
    }
}

enum OptionName<'a> {
    Dash(~[&'a str]),
    Pos(&'a str),
}

struct GenericOption<'a> {
    name: OptionName<'a>,
    help: &'a str,
    action: Action,
}

pub struct Context<'a, 'b> {
    parser: &'a ArgumentParser<'b>,
    iter: Items<'a, ~str>,
}

impl<'a, 'b> Context<'a, 'b> {

    fn get_next_option(&mut self, action: &IArgAction, name: &str)
        -> ParseResult
    {
        let value = self.iter.next();
        match value {
            Some(arg) => {
                let argborrow: &str = *arg;
                return action.parse_arg(argborrow);
            }
            None => {
               return Error(format!("Option {} requires value", name));
            }
        }
    }

    fn parse_long_option<'c>(&'c mut self, arg: &str) -> ParseResult {
        let mut equals_iter = arg.splitn('=', 1);
        let optname = match equals_iter.next() {
            Some(value) => { value }
            None => { fail!() }
        };
        let valueref = equals_iter.next();
        let opt = self.parser.long_options.find(&optname.to_str());
        match opt {
            Some(opt) => {
                match opt.action {
                    Flag(ref action) => {
                        match valueref {
                            Some(_) => {
                                return Error(format!(
                                    "Option {} does not accept an argument",
                                    optname));
                            }
                            None => {
                                return action.parse_flag();
                            }
                        }
                    }
                    Single(ref action) => {
                        match valueref {
                            Some(value) => {
                                return action.parse_arg(value);
                            }
                            None => {
                                return self.get_next_option(
                                    *action, optname);
                            }
                        }
                    }
                    _ => { fail!("Not Implemented") }
                }
            }
            None => {
                return Error(format!("Unknown option {}", arg));
            }
        }
    }

    fn parse_short_options<'c>(&'c mut self, arg: &str) -> ParseResult {
        let mut iter = arg.char_indices();
        iter.next();
        for (idx, ch) in iter {
            let opt = match self.parser.short_options.find(&ch) {
                Some(opt) => { opt }
                None => {
                    return Error(format!("Unknown short option \"{}\"", ch));
                }
            };
            let res = match opt.action {
                Flag(ref action) => action.parse_flag(),
                Single(ref action) => {
                    if idx + 1 < arg.len() {
                        return action.parse_arg(arg.slice(idx+1, arg.len()));
                    } else {
                        return self.get_next_option(*action, arg);
                    }
                }
                _ => { fail!("Not Implemented"); }
            };
            match res {
                Parsed => { continue; }
                x => { return x; }
            }
        }
        return Parsed;
    }

    fn parse(parser: &ArgumentParser, args: &[~str])
        -> ParseResult
    {
        let mut ctx = Context {
            parser: parser,
            iter: args.iter(),
        };
        ctx.iter.next();  // Command name
        loop {
            let next = ctx.iter.next();
            let arg = match next {
                Some(arg) => { arg }
                None => { break; }
            };
            let res = match ArgumentKind::check(*arg) {
                Positional => { fail!() } //ctx.parse_argument(*arg) }
                LongOption => { ctx.parse_long_option(*arg) }
                ShortOption => { ctx.parse_short_options(*arg) }
            };
            match res {
                Parsed => continue,
                _ => return res,
            }
        }
        return Parsed;
    }
}

pub struct Ref<'a, 'b, T> {
    priv cell: Rc<RefCell<&'a mut T>>,
    priv parser: &'a mut ArgumentParser<'b>,
}

impl<'a, 'b, T> Ref<'a, 'b, T> {

    pub fn add_option<'x>(&'x mut self, names: ~[&'b str],
        help: &'b str, action: ~TypedAction<T>) -> &'x mut Ref<'a, 'b, T>
    {
        let opt = Rc::new(GenericOption {
            name: Dash(names.clone()),
            help: help,
            action: action.bind(self.cell.clone()),
            });

        for nameptr in names.iter() {
            let name = *nameptr;
            match ArgumentKind::check(name) {
                Positional => { fail!("Bad argument name {}", name); }
                LongOption => {
                    self.parser.long_options.insert(
                        name.to_str(), opt.clone());
                }
                ShortOption => {
                    if name.len() > 2 {
                        fail!("Bad short argument {}", name);
                    }
                    self.parser.short_options.insert(
                        name[1] as char, opt.clone());
                }
            }
        }
        self.parser.options.push(opt);
        return self;
    }
    /*
    fn add_argument<'b>(&'b mut self, name: &'a str,
        help: &'a str, action: Action<'a>) {
        self.arguments.push(Argument {
            name: name,
            help: help,
            action: action,
        })
    }
    */
}

pub struct ArgumentParser<'a> {
    priv options: ~[Rc<GenericOption<'a>>],
    priv arguments: ~[Rc<GenericOption<'a>>],
    priv short_options: HashMap<char, Rc<GenericOption<'a>>>,
    priv long_options: HashMap<~str, Rc<GenericOption<'a>>>,
}



impl<'a> ArgumentParser<'a> {

    pub fn new() -> ArgumentParser {
        return ArgumentParser {
            arguments: ~[],
            options: ~[],
            short_options: HashMap::new(),
            long_options: HashMap::new(),
            };
    }

    pub fn refer<'x, T>(&'x mut self, val: &'x mut T)
        -> ~Ref<'x, 'a, T>
    {
        return ~Ref {
            cell: Rc::new(RefCell::new(val)),
            parser: self,
        };
    }

    pub fn parse_list(&self, args: ~[~str]) -> Result<(), int> {
        match Context::parse(self, args) {
            Parsed => return Ok(()),
            Exit => return Err(0),
            Error(val) => {
                self.error(args[0], val);
                return Err(2);
            }
        }
    }

    fn error(&self, command: &str, message: &str) {
        println!("{}: {}", command, message);
    }

    pub fn parse_args(&self) -> Result<(), int> {
        return self.parse_list(os::args());
    }
}
