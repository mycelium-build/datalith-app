//! Static validation for parsed Base expressions.

use anyhow::{Result, bail};

use super::{ArithOp, Expr, ValType, coerce_duration};

#[derive(Default)]
pub(super) struct Validator;

impl Validator {
    pub(super) fn walk(&mut self, expression: &Expr) -> Result<()> {
        match expression {
            Expr::Null | Expr::Bool(_) | Expr::Number(_) | Expr::Property(_) => Ok(()),
            Expr::Text(_) => {
                // Bare text is fine; duration-looking text is only consumed
                // where dates meet strings.
                Ok(())
            }
            Expr::Not(inner) | Expr::Neg(inner) => self.walk(inner),
            Expr::Arithmetic { op, left, right } => {
                self.walk(left)?;
                self.walk(right)?;
                Self::check_date_arithmetic(*op, left, right)
            }
            Expr::Compare { left, right, .. } | Expr::Logic { left, right, .. } => {
                self.walk(left)?;
                self.walk(right)
            }
            Expr::Call(name, args) => self.check_call(name, args),
            Expr::Method {
                subject,
                name,
                args,
            } => {
                self.walk(subject)?;
                args.iter().try_for_each(|arg| self.walk(arg))?;
                Self::check_method(subject, name, args)
            }
        }
    }

    fn check_date_arithmetic(op: ArithOp, left: &Expr, right: &Expr) -> Result<()> {
        let left_type = left.infer();
        let right_type = right.infer();
        let left_duration = coerce_duration(left);
        let right_duration = coerce_duration(right);
        if left_duration.is_some() || right_duration.is_some() {
            let other_is_date = if right_duration.is_some() {
                left_type == ValType::Date
            } else {
                right_type == ValType::Date
            };
            if !other_is_date {
                bail!("duration arithmetic requires a date value such as file.mtime or now()");
            }
            if left_duration.is_some() && op == ArithOp::Subtract {
                bail!("durations can only be added on the left; place the duration on the right");
            }
            return Ok(());
        }
        let date_involved = left_type == ValType::Date || right_type == ValType::Date;
        if !date_involved {
            return Ok(());
        }
        if op == ArithOp::Subtract && left_type == ValType::Date && right_type == ValType::Date {
            return Ok(());
        }
        if left_type == ValType::Date && right_type == ValType::Date && op == ArithOp::Add {
            bail!("two dates cannot be added");
        }
        bail!("date arithmetic requires a duration literal such as \"1M\" on the other side")
    }

    #[allow(clippy::match_same_arms)]
    fn check_call(&mut self, name: &str, args: &[Expr]) -> Result<()> {
        match name {
            "file.hasTag" | "file.hasLink" | "file.inFolder" => {
                if args.len() != 1 {
                    bail!("{name} takes exactly one argument");
                }
                let Some(Expr::Text(_)) = args.first() else {
                    bail!("{name} requires a string literal argument");
                };
                Ok(())
            }
            "today" | "now" => {
                if !args.is_empty() {
                    bail!("{name} takes no arguments");
                }
                Ok(())
            }
            "date" | "datetime" | "time" | "julianday" | "abs" | "round" | "length" | "lower"
            | "upper" | "trim" | "ltrim" | "rtrim" | "instr" => {
                if args.len() != 1 {
                    bail!("{name} takes exactly one argument");
                }
                let Some(first) = args.first() else {
                    bail!("{name} takes exactly one argument");
                };
                self.walk(first)
            }
            "if" => {
                if args.len() != 3 {
                    bail!("if takes exactly three arguments");
                }
                args.iter().try_for_each(|arg| self.walk(arg))
            }
            "min" | "max" => {
                if args.is_empty() {
                    bail!("{name} takes at least one argument");
                }
                args.iter().try_for_each(|arg| self.walk(arg))
            }
            "replace" | "substr" => {
                if args.len() != 3 {
                    bail!("{name} takes exactly three arguments");
                }
                args.iter().try_for_each(|arg| self.walk(arg))
            }
            _ => bail!("unsupported function {name:?}"),
        }
    }

    fn check_method(subject: &Expr, name: &str, args: &[Expr]) -> Result<()> {
        let arity_ok = match name {
            "contains" | "toFixed" | "round" | "startsWith" | "endsWith" | "format" => {
                args.len() == 1
            }
            "lower" | "upper" | "trim" | "abs" | "date" | "mean" | "min" | "max" | "sum"
            | "count" => args.is_empty(),
            _ => false,
        };
        if !arity_ok {
            bail!("unsupported method .{name} with {} argument(s)", args.len());
        }
        if matches!(name, "toFixed" | "round") && !matches!(args.first(), Some(Expr::Number(_))) {
            bail!(".{name} requires a numeric literal argument");
        }
        if name == "format" && !matches!(args.first(), Some(Expr::Text(_))) {
            bail!(".format requires a string literal format");
        }
        let _ = subject;
        Ok(())
    }
}
