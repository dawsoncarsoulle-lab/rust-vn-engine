//! Getter authority is checked at lookup; the resulting class is independent
//! of the instance lifetime. No object registry or class catalog is duplicated.
use super::WidgetValueAdmission;
use rvn_parser::{Value,widget_reference::WidgetReference};

impl WidgetValueAdmission {
    pub(crate) fn object_class(&self, reference: &WidgetReference) -> Result<Value,String> {
        if reference.portable().is_none() { return Ok(Value::WidgetClass(Box::new(None))); }
        // target() checks the exact current resident identity, lineage and epoch.
        self.target(reference)?;
        let portable=reference.portable().ok_or("Widget reference is null")?;
        let class=crate::widget_class_getter_0981::resolve_class_data(&self.classes,Some(portable.class().uuid()))?;
        Ok(Value::WidgetClass(Box::new(class)))
    }
}

pub(crate) fn evaluate(admission:Option<&WidgetValueAdmission>, value:&Value)->Result<Value,String>{
    let Value::Widget(reference)=value else { return Err("Get Class requires a typed Widget object, not a class, string or untyped null".into()); };
    if reference.portable().is_none(){return Ok(Value::WidgetClass(Box::new(None)));}
    admission.ok_or("Get Class requires its actual Engine object context")?.object_class(reference)
}
