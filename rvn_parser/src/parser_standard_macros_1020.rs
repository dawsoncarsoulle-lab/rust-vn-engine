//! Child of Parser. Dedicated typed AST; no ScheduledConsumer wrapper.
use super::*;
use crate::scheduled_source::standard_macro_contract::{
    Branch, Control, DataOutput, Dispatch, Kind, Output, Site,
};
impl Parser<'_> {
    pub(super) fn parse_standard_macro(&mut self) -> ParseResult<Statement> {
        let loc = self.current_location();
        if self.handler_depth == 0 || self.function_depth != 1 {
            return Err(self.err_msg(
                loc,
                "macro_dispatch_v1".into(),
                "un véritable Handler Widget (première tranche)",
            ));
        }
        let literal = |parser: &mut Self| -> ParseResult<String> {
            match parser.parse_expr()? {
                Expr::Str(value)
                    if !value.is_empty() && value.len() <= 256 && !value.contains('\0') =>
                {
                    Ok(value)
                }
                _ => Err(parser.err_msg(
                    loc,
                    "macro identity".into(),
                    "une identité source littérale bornée",
                )),
            }
        };
        let scope = literal(self)?;
        self.expect(",")?;
        let node = literal(self)?;
        self.expect(",")?;
        let control = match literal(self)?.as_str() {
            "enter" => Control::Enter,
            "reset" => Control::Reset,
            "open" => Control::Open,
            "close" => Control::Close,
            "toggle" => Control::Toggle,
            value => {
                return Err(self.err_msg(
                    loc,
                    value.into(),
                    "un port de contrôle StandardMacro typé",
                ))
            }
        };
        let site = Site::decode(&scope, &node)
            .map_err(|error| self.err_msg(loc, error, "les vrais root/node/pins du Handler"))?;
        let kind = site
            .kind()
            .map_err(|error| self.err_msg(loc, error, "StandardMacro"))?;
        kind.control_role(control)
            .map_err(|error| self.err_msg(loc, error, "le vrai contrôle du macro"))?;
        let mut deferred_operand_v1 = None;
        let operand = if kind.operand_required(control) {
            self.expect(",")?;
            if matches!(self.peek(), Some(Token::Ident("operand_v1"))) {
                self.advance(); self.expect("{")?; self.skip_newlines();
                let mut steps = Vec::new();
                while matches!(self.peek(), Some(Token::Ident("capture" | "inspect"))) {
                    let inspect = matches!(self.advance(), Some(Token::Ident("inspect")));
                    let producer = literal(self)?; self.expect("=")?;
                    let value = self.parse_expr()?;
                    if steps.len() >= 1024 { return Err(self.err_msg(loc, "operand_v1".into(), "1024 producteurs au maximum")); }
                    steps.push(if inspect { crate::ConsumerStep::InspectTextTable { producer, value } } else { crate::ConsumerStep::Capture { producer, value } });
                    self.skip_newlines();
                }
                self.expect("}")?; self.expect("=>")?;
                deferred_operand_v1 = Some(crate::scheduled_source::standard_macro_contract::DeferredOperandV1 { steps });
            }
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(")")?;
        self.expect("{")?;
        self.skip_newlines();
        let outputs: &[(Output, &str)] = if control != Control::Enter {
            &[]
        } else {
            match kind {
                Kind::DoOnce => &[(Output::Completed, "completed")],
                Kind::DoN | Kind::Gate => &[(Output::Exit, "exit")],
                Kind::FlipFlop => &[(Output::A, "a"), (Output::B, "b")],
            }
        };
        let mut branches = Vec::new();
        for &(output, role) in outputs {
            self.expect("{")?;
            branches.push(Branch {
                output,
                pin: site.node.ports.iter().find(|p| p.role == role).unwrap().pin,
                body: self.parse_block()?,
            });
            self.skip_newlines();
        }
        self.expect("}")?;
        let dispatch = Dispatch {
            site,
            control,
            operand,
            deferred_operand_v1,
            branches,
        };
        dispatch
            .validate()
            .map_err(|error| self.err_msg(loc, error, "un dispatchmacro au port réel"))?;
        Ok(Statement::StandardMacroDispatch {
            dispatch: Box::new(dispatch),
        })
    }
    pub(super) fn typed_macro_read(&self, name: &str, args: &[Expr]) -> ParseResult<Option<Expr>> {
        let output = match name {
            "flow.macro_counter_v1" => DataOutput::Counter,
            "flow.macro_is_a_v1" => DataOutput::IsA,
            _ => return Ok(None),
        };
        let loc = self.current_location();
        let [Expr::Str(scope), Expr::Str(node)] = args else {
            return Err(self.err_msg(loc, name.into(), "root/node source littéraux exacts"));
        };
        let site = Site::decode(scope, node)
            .map_err(|error| self.err_msg(loc, error, "le vrai site macro"))?;
        let pin = site
            .data_pin(output)
            .map_err(|error| self.err_msg(loc, error, "la vraie sortie data macro"))?;
        Ok(Some(Expr::StandardMacroData {
            site: Box::new(site),
            output,
            pin,
        }))
    }
}
