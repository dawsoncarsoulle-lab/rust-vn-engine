//! Child of the actual Parser, reused by the proposed whole-program hooks.
//! Standalone entry points validate transport now without activating an AST
//! variant/Engine route or changing parse() for an existing valid script.
use super::*;
use crate::function_library_asset1023::{FunctionLibraryId,QualifiedLibraryFunction, MAX_FUNCTION_LIBRARY_BYTES,MAX_FUNCTION_LIBRARY_MEMBERS,MAX_FUNCTION_LIBRARY_PARAMETERS};
use crate::function_library_program1023::{FunctionLibraryScope,LibraryMemberSource,LibraryParameterId,QualifiedLibraryCall,LibraryCallMode};

impl Parser<'_>{
    fn library_literal1023(&mut self)->ParseResult<String>{
        let loc=self.current_location();
        match self.parse_expr()?{Expr::Str(value)=>Ok(value),_=>Err(self.err_msg(loc,"Library identity".into(),"une identité source littérale"))}
    }
    fn library_graph_id1023(&mut self)->ParseResult<u64>{
        let loc=self.current_location();let raw=self.library_literal1023()?;
        raw.parse::<u64>().ok().filter(|id|*id>0&&id.to_string()==raw)
            .ok_or_else(||self.err_msg(loc,raw,"un GraphId source non nul, décimal canonique entre guillemets"))
    }
    /// Called just after functions.library_v1( in the same Parser. Root scope
    /// only; inner actual function parsing retains its original lexical rules.
    pub(super) fn parse_function_library_scope1023(&mut self)->ParseResult<FunctionLibraryScope>{
        let loc=self.current_location();
        if self.function_depth!=0||self.block_depth!=0||self.init_depth!=0{return Err(self.err_msg(loc,"functions.library_v1".into(),"une déclaration Library à la racine du Source"));}
        let id=FunctionLibraryId::parse(self.library_literal1023()?).map_err(|error|self.err_msg(loc,error,"une vraie identité Library"))?;
        self.expect(")")?;self.expect("{")?;let mut members=Vec::new();
        while !matches!(self.peek(),Some(Token::BraceClose)){
            if members.len()>=MAX_FUNCTION_LIBRARY_MEMBERS{return Err(self.err_msg(loc,"Library member count".into(),"au plus 65536 membres Library"));}
            if self.expect_ident("functions")?!="functions"{return Err(self.err_msg(self.current_location(),"Library member".into(),"functions.member_v1"));}
            self.expect(".")?;
            if self.expect_ident("member_v1")?!="member_v1"{return Err(self.err_msg(self.current_location(),"Library member".into(),"functions.member_v1"));}
            self.expect("(")?;let graph=self.library_graph_id1023()?;self.expect(",")?;
            let parameter_loc=self.current_location();self.expect("[")?;let mut ids=Vec::new();
            while !matches!(self.peek(),Some(Token::BracketClose)){
                if ids.len()>=MAX_FUNCTION_LIBRARY_PARAMETERS{return Err(self.err_msg(parameter_loc,"parameter count".into(),"128 paramètres au maximum"));}
                let id=LibraryParameterId::parse(self.library_literal1023()?).map_err(|error|self.err_msg(parameter_loc,error,"une vraie identité de paramètre"))?;
                ids.push(id);
                if !matches!(self.peek(),Some(Token::Comma)){break;}
                self.expect(",")?;
            }
            self.expect("]")?;
            self.expect(")")?;self.expect("{")?;
            if !matches!(self.peek(),Some(Token::Function)){return Err(self.err_msg(self.current_location(),"Library body".into(),"une seule déclaration function réelle"));}
            // Do not enter a fake computation/body block around the declaration.
            // parse_function opens/closes its real original lexical body itself.
            let declaration=self.parse_statement()?;self.expect("}")?;
            members.push(LibraryMemberSource::new(graph,ids,declaration).map_err(|error|self.err_msg(loc,error,"un membre Library cohérent"))?);
        }
        self.expect("}")?;
        FunctionLibraryScope::new(id,members).map_err(|error|self.err_msg(loc,error,"un scope Library nominal exact"))
    }
    /// Called after functions.value_v1( / invoke_v1(. No argument is evaluated.
    pub(super) fn parse_function_library_call1023(&mut self,mode:LibraryCallMode)->ParseResult<(LibraryCallMode,QualifiedLibraryCall)>{
        let loc=self.current_location();
        let library=FunctionLibraryId::parse(self.library_literal1023()?).map_err(|error|self.err_msg(loc,error,"une vraie identité Library"))?;
        self.expect(",")?;let graph_id=self.library_graph_id1023()?;self.expect(",")?;
        self.expect("{")?;let mut arguments=Vec::new();
        while !matches!(self.peek(),Some(Token::BraceClose)){
            if arguments.len()>=MAX_FUNCTION_LIBRARY_PARAMETERS{return Err(self.err_msg(loc,"argument count".into(),"128 arguments au maximum"));}
            let id=LibraryParameterId::parse(self.library_literal1023()?).map_err(|error|self.err_msg(loc,error,"un vrai ParameterId"))?;
            self.expect(":")?;let expression=self.parse_expr()?;
            arguments.push((id,expression));
            if !matches!(self.peek(),Some(Token::Comma)){break;}
            self.expect(",")?;
        }
        self.expect("}")?;
        self.expect(")")?;
        let call=QualifiedLibraryCall::new(QualifiedLibraryFunction{library,graph_id},arguments).map_err(|error|self.err_msg(loc,error,"un appel Library nominal exact"))?;
        Ok((mode,call))
    }
}
fn header(parser:&mut Parser<'_>)->ParseResult<String>{
    let loc=parser.current_location();
    if parser.expect_ident("functions")?!="functions"{return Err(parser.err_msg(loc,"scope/call".into(),"le namespace functions versionné"));}
    parser.expect(".")?;let method=parser.expect_ident("méthode functions versionnée")?;parser.expect("(")?;Ok(method)
}
fn end(parser:&mut Parser<'_>)->ParseResult<()>{
    parser.skip_newlines();if parser.peek().is_some(){return Err(parser.err_msg(parser.current_location(),"source résiduel".into(),"une seule déclaration ou un seul appel Library"));}Ok(())
}
fn check_source_bytes(source:&str)->ParseResult<()>{
    if source.len()>MAX_FUNCTION_LIBRARY_BYTES{
        // Do not copy an attacker-sized first line into an error before lexing.
        return Err(ParseError::build(ParseErrorKind::UnexpectedToken{got:"Library Source byte limit".into(),expected:"au plus 16 MiB de Source"},SourceLocation{line:1,col:1,len:0},""));
    }
    Ok(())
}
pub fn parse_library_scope(source:&str)->ParseResult<FunctionLibraryScope>{
    check_source_bytes(source)?;
    let mut parser=Parser::new(source)?;
    if header(&mut parser)?!="library_v1"{return Err(parser.err_msg(parser.current_location(),"Library scope".into(),"functions.library_v1"));}
    let scope=parser.parse_function_library_scope1023()?;end(&mut parser)?;Ok(scope)
}
pub fn parse_library_call(source:&str)->ParseResult<(LibraryCallMode,QualifiedLibraryCall)>{
    check_source_bytes(source)?;
    let mut parser=Parser::new(source)?;
    let mode=match header(&mut parser)?.as_str(){"value_v1"=>LibraryCallMode::Value,"invoke_v1"=>LibraryCallMode::Completion,other=>return Err(parser.err_msg(parser.current_location(),other.into(),"functions.value_v1 ou functions.invoke_v1"))};
    let call=parser.parse_function_library_call1023(mode)?;end(&mut parser)?;Ok(call)
}
