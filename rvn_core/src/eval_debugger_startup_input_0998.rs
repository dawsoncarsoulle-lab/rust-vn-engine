//! Exact frozen Imagemap input record validation. Loop admission stays separate.
use super::*;
impl FunctionLibrary {
    pub(crate) fn validate_retained_imagemap_input(&self,statement:&Statement,scope:&str,route:&[Route],records:&[PortableRecord],locale:Option<&crate::locale::LocaleManager>)->EvalResult<Value>{
        if !matches!(statement,Statement::ScheduledConsumer{version:2,body,..}if matches!(body.as_slice(),[Statement::Imagemap{..}])){
            return Err(invalid("Retained Init input is not its actual frozen Imagemap v2 header"));
        }
        let [record]=records else{return Err(invalid("Retained Init Imagemap requires its one actual completed input root"));};
        record.validate_loop_data()?;
        let root=Root::Consumer(statement.clone(),scope.into());
        if record.request.route_prefix!=route{return Err(invalid("Retained Init Imagemap belongs to another actual source route"));}
        // Only the source payload is omitted by portable serialization. A live
        // record must already match, and a cold record is rebuilt from source.
        if !matches!(record.request.root,Root::Block(ref body)if body.is_empty())&&record.request.root!=root{
            return Err(invalid("Retained live input root differs from its actual frozen Imagemap"));
        }
        let mut row=record.clone();row.request.root=root;
        rebind_state(&mut row.request.state,self,locale);rebind_state(&mut row.result.state,self,locale);
        let mut library=self.clone();library.debug_probe=None;
        let current=Continuation::new(library,row.request.clone())?;
        let probe=Probe::shared();
        probe.lock().map_err(|_|invalid("Imagemap input validation lock unavailable"))?.requested=Some(current.request.clone());
        Probe::restore_record_with_scope(&probe,&current,&row,locale,None)?;
        let value=row.result.value.ok_or_else(||invalid("Retained Imagemap has no completed header value"))?;
        if !matches!(&value,Value::List(fields)if fields.is_empty()){
            return Err(invalid("Retained static Imagemap has foreign computed input fields"));
        }
        Ok(value)
    }
}
