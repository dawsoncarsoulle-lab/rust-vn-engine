//! Current owner admission, borrowed by the existing VM. This creates neither
//! a Widget instance nor a backend wrapper and does not copy FunctionLibrary.
use super::*;
use crate::widget_reference_values_0974::{WidgetValueAdmission,object_class};
impl<R:Renderer> Engine<R>{
    pub fn widget_object_class(&self,value:&Value)->Result<Value,RuntimeError>{
        if !matches!(value,Value::Widget(reference) if !reference.is_null()){
            return object_class::evaluate(None,value).map_err(|error|self.scene_resource_error(error));
        }
        let plan=WidgetValueAdmission::new(&self.state,self.widget_classes.clone(),self.instance_id(),self.runtime_epoch()).map_err(|error|self.scene_resource_error(error))?;
        object_class::evaluate(Some(&plan),value).map_err(|error|self.scene_resource_error(error))
    }
    pub(super) fn widget_object_context(&self,state:&GameState)->Result<Option<WidgetValueAdmission>,RuntimeError>{
        if !self.functions.uses_widget_object_class{return Ok(None);}
        self.widget_object_context_required(state).map(Some)
    }
    pub(super) fn widget_object_context_required(&self,state:&GameState)->Result<WidgetValueAdmission,RuntimeError>{
        WidgetValueAdmission::new(state,self.widget_classes.clone(),self.instance_id(),self.runtime_epoch()).map_err(|error|self.scene_resource_error(error))
    }
}
