//! Pure typed class data from the existing immutable, source-admitted catalog.
//! This module does not allocate an instance, execute Init or grant runtime input.
use crate::widget_class_source_0950::AdmittedWidgetClasses;
use rvn_parser::widget_reference::WidgetClassReference;

/// `None` is a nullable class value. It is not an unresolved UUID and not a
/// Widget object reference. A missing requested class always remains an error.
/// The caller supplies its current source-owned catalog, never a decoded DTO.
pub(crate) fn resolve_class_data(
    classes: &AdmittedWidgetClasses,
    uuid: Option<&str>,
) -> Result<Option<WidgetClassReference>, String> {
    let Some(uuid) = uuid else {
        return Ok(None);
    };
    let class = classes
        .class(uuid)
        .ok_or("Widget class getter has no admitted source class")?;
    let identity = class.identity();
    if identity.uuid != uuid {
        return Err("Widget class getter differs from the actual index identity".into());
    }
    Ok(Some(WidgetClassReference::new(
        identity.uuid.clone(),
        identity.receipt,
    )?))
}

/// The same admission rule for an existing class datum, including a stopped or
/// decoded value. Class data has no Engine epoch or resident-instance authority.
pub(crate) fn validate_class_data(
    classes: &AdmittedWidgetClasses,
    value: Option<&WidgetClassReference>,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    value.validate()?;
    let actual = resolve_class_data(classes, Some(value.uuid()))?
        .ok_or("A non-null Widget class did not resolve to admitted class data")?;
    if actual != *value {
        return Err(
            "Widget class getter value differs from its current admitted source receipt".into(),
        );
    }
    Ok(())
}

/// Validate only class data, including nested retained values. Object residency
/// remains the responsibility of the existing WidgetValueAdmission boundary.
pub(crate) fn validate_class_values(classes: &AdmittedWidgetClasses, value: &rvn_parser::Value) -> Result<(), String> {
    crate::value_limits::value_work(value).map_err(|error| error.to_string())?;
    fn walk(classes: &AdmittedWidgetClasses, value: &rvn_parser::Value) -> Result<(), String> {
        match value {
            rvn_parser::Value::WidgetClass(class) => validate_class_data(classes, class.as_ref().as_ref()),
            rvn_parser::Value::List(values) => { for value in values { walk(classes, value)?; } Ok(()) },
            rvn_parser::Value::Dict(values) => { for value in values.values() { walk(classes, value)?; } Ok(()) },
            _ => Ok(()),
        }
    }
    walk(classes, value)
}
