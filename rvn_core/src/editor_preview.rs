//! Disposable editor transport, outside GameState, checkpoints and source files.
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const WIRE_VERSION: u32 = 6;
pub const MAX_HEADER_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_INSPECTION_BYTES:usize=512*1024;
pub const MAX_INPUT_BYTES: usize = 16 * 1024;
/// An image input carries data, never a fresh UI capability. The receiver first
/// admits its retained completed-image context and only then decodes this enum.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum ImageInput {
    Pointer { kind: PointerKind, button: u8, point: [f32; 2], wheel: [f32; 2] },
    Key { key: String, logical:Option<String>, pressed: bool, modifiers: [bool; 4] },
    Text { value: String, composition: Option<[usize; 2]> },
    FocusLost,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PointerKind { Down, Up, Move, Wheel }
impl ImageInput {
    pub fn is_passive(&self)->bool {
        matches!(self,Self::FocusLost|Self::Key{pressed:false,..}|Self::Pointer{kind:PointerKind::Up,..})
    }
    pub fn encode(&self) -> Result<String, String> {
        let value=serde_json::to_string(self).map_err(|e|e.to_string())?;
        Self::decode(&value)?; Ok(value)
    }
    pub fn decode(value:&str) -> Result<Self,String> {
        if value.len()>MAX_INPUT_BYTES {return Err("Oversized image input".into());}
        let input:Self=serde_json::from_str(value).map_err(|e|e.to_string())?;
        match &input {
            Self::Pointer{button,point,wheel,..} if *button>2 || point.iter().chain(wheel).any(|v|!v.is_finite() || v.abs()>65536.0)=>return Err("Invalid image pointer".into()),
            Self::Key{key,logical,..} if key.is_empty() || key.len()>32 || logical.as_ref().is_some_and(|value|value.len()>8 || value.chars().count()!=1)=>return Err("Invalid image key".into()),
            Self::Text{value,composition} if value.len()>8192 || composition.is_some_and(|range|range[0]>range[1] || range[1]>value.chars().count())=>return Err("Invalid image composition".into()),
            _=>{}
        }
        Ok(input)
    }
}
// Private independent proposal. Not installed or runtime-qualified.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum ImageRelease {
    Pointer { button: u8, point: [f32; 2], wheel: [f32; 2] },
    Key { key: String, logical: Option<String>, modifiers: [bool; 4] },
    FocusLost,
}
impl ImageRelease {
    pub fn into_input(self) -> ImageInput {
        match self {
            Self::Pointer { button, point, wheel } => ImageInput::Pointer { kind: PointerKind::Up, button, point, wheel },
            Self::Key { key, logical, modifiers } => ImageInput::Key { key, logical, pressed: false, modifiers },
            Self::FocusLost => ImageInput::FocusLost,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Pointer { button, point, wheel } if *button > 2 || point.iter().chain(wheel).any(|v| !v.is_finite() || v.abs() > 65536.0) => Err("Invalid image release pointer".into()),
            Self::Key { key, logical, .. } if key.is_empty() || key.len() > 32 || logical.as_ref().is_some_and(|value| value.len() > 8 || value.chars().count() != 1) => Err("Invalid image release key".into()),
            _ => Ok(()),
        }
    }
}
/// Actual producer and CPU admission fixture use the same bounded envelope.
/// This never upgrades the original completed-image identity to a newer scope.
pub fn image_input_header(scope: &FrameScope, sequence: u64, input: ImageInput) -> Result<Header, String> {
    // Preserve the existing bounds for every input before constructing a header.
    let payload = input.encode()?;
    let release = match input {
        ImageInput::Pointer { kind: PointerKind::Up, button, point, wheel } => Some(ImageRelease::Pointer { button, point, wheel }),
        ImageInput::Key { key, logical, pressed: false, modifiers } => Some(ImageRelease::Key { key, logical, modifiers }),
        ImageInput::FocusLost => Some(ImageRelease::FocusLost),
        _ => None,
    };
    Ok(if let Some(release) = release {
        Header::Release { instance: scope.instance, epoch: scope.epoch, sequence, frame_sequence: scope.sequence, release }
    } else {
        Header::Input { instance: scope.instance, epoch: scope.epoch, sequence, frame_sequence: scope.sequence, input: payload }
    })
}

pub const MAX_WIDTH: u32 = 2560;
pub const MAX_HEIGHT: u32 = 1440;
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlKind {
    Pause,
    Resume,
    Step,
    Advance,
    Stop,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivationScope {
    pub activation: u64,
    pub scene: String,
}
/// Transport the engine's existing display generation/history visit, without
/// saving a new ID or selecting a later interaction while draining a command.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionSource {
    pub instance: u64,
    pub epoch: u64,
    pub position: crate::scene::ResumePosition,
    pub generation: Option<u64>,
    pub history_entry: Option<u64>,
    /// The existing Core authored destinations prevent a changed conditional
    /// list from reusing a visible index at the same visit.
    pub choice_indices: Vec<usize>,
    pub owner: Option<crate::scene::ActivationId>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameScope {
    pub instance: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub control_sequence: u64,
    /// Last command handled (including an exact refusal), tied to this image.
    pub control_ack: u64,
    pub control_error: Option<String>,
    pub paused: bool,
    pub publication_pending: bool,
    /// Core termination observed for this current image; unknown while publication is pending or refused.
    /// Optional observation only; it grants no input, control or debugger authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished: Option<bool>,
    pub step_allowed: bool,
    pub interaction: Option<InteractionSource>,
    pub position: crate::scene::ResumePosition,
    pub active: Option<ActivationScope>,
    pub parked: Vec<ActivationScope>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum Header {
    Hello {
        version: u32,
        nonce: u64,
        pid: u32,
        instance: u64,
    },
    SessionOffer {nonce:u64,pid:u32,instance:u64,next_instance:u64},
    SessionGrant {nonce:u64,instance:u64,next_instance:u64,next_nonce:u64},
    /// Actual Main consumption on this authenticated stream. This is transport
    /// credit only, never an input permit or a rendered control acknowledgement.
    FrameConsumed {instance:u64,epoch:u64,frame_sequence:u64},
    Control {
        instance: u64,
        epoch: u64,
        sequence: u64,
        frame_control_sequence: u64,
        position: crate::scene::ResumePosition,
        interaction: Option<InteractionSource>,
        kind: ControlKind,
    },
    Input {
        instance: u64,
        epoch: u64,
        sequence: u64,
        frame_sequence: u64,
        input: String,
    },
    /// Release authority can only retire its original known image or route
    /// a passive input to its still-current completed-image recipient.
    Release {instance:u64,epoch:u64,sequence:u64,frame_sequence:u64,release:ImageRelease},
    /// Command body stays opaque until the retained completed image and its
    /// current Engine/visit/control sequence have been admitted.
    Debugger {instance:u64,epoch:u64,sequence:u64,frame_sequence:u64,frame_control_sequence:u64,command:String},
    // A control is acknowledged by this actual rendered frame, never by an
    // immediate synthetic image or a state-only acknowledgement.
    Frame {
        scope: FrameScope,
        width: u32,
        height: u32,
        #[serde(default,skip_serializing_if="Option::is_none")]
        debugger:Option<String>,
        #[serde(default,skip_serializing_if="Option::is_none")]
        inspection:Option<String>,
    },
}
#[derive(Clone, Debug)]
pub struct Packet {
    pub header: Header,
    pub pixels: Vec<u8>,
}
pub fn image_bytes(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
        return Err("Editor image dimensions exceed the bounded transport".into());
    }
    Ok(width as usize * height as usize * 4)
}
impl Packet {
    pub fn header(header: Header) -> Self {
        Self {
            header,
            pixels: Vec::new(),
        }
    }
    pub fn read(reader: &mut impl Read) -> Result<Self, String> {
        let mut length = [0; 4];
        reader.read_exact(&mut length).map_err(|e| e.to_string())?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 || length > MAX_HEADER_BYTES {
            return Err("Invalid editor header length".into());
        }
        let mut encoded = vec![0; length];
        reader.read_exact(&mut encoded).map_err(|e| e.to_string())?;
        let header: Header = serde_json::from_slice(&encoded).map_err(|e| e.to_string())?;
        if matches!(&header,Header::Input{input,..} if input.len()>MAX_INPUT_BYTES) {
            return Err("Oversized image input envelope".into());
        }
        if matches!(&header,Header::Debugger{command,..}if command.len()>crate::editor_debugger::MAX_COMMAND_BYTES)
            ||matches!(&header,Header::Frame{debugger:Some(snapshot),..}if snapshot.len()>crate::editor_debugger::MAX_SNAPSHOT_BYTES)
            ||matches!(&header,Header::Frame{inspection:Some(snapshot),..}if snapshot.len()>MAX_INSPECTION_BYTES){return Err("Oversized editor envelope".into());}
        if let Header::Release{release,..}=&header {release.validate()?;}
        let count = match &header {
            Header::Frame { width, height, .. } => image_bytes(*width, *height)?,
            _ => 0,
        };
        let mut pixels = vec![0; count];
        reader.read_exact(&mut pixels).map_err(|e| e.to_string())?;
        Ok(Self { header, pixels })
    }
    pub fn write(&self, writer: &mut impl Write) -> Result<(), String> {
        let count = match &self.header {
            Header::Frame { width, height, .. } => image_bytes(*width, *height)?,
            _ => 0,
        };
        if self.pixels.len() != count {
            return Err("Editor pixels do not match the exact frame dimensions".into());
        }
        if matches!(&self.header,Header::Debugger{command,..}if command.len()>crate::editor_debugger::MAX_COMMAND_BYTES)
            ||matches!(&self.header,Header::Frame{debugger:Some(snapshot),..}if snapshot.len()>crate::editor_debugger::MAX_SNAPSHOT_BYTES)
            ||matches!(&self.header,Header::Frame{inspection:Some(snapshot),..}if snapshot.len()>MAX_INSPECTION_BYTES){return Err("Oversized editor envelope".into());}
        if let Header::Release{release,..}=&self.header {release.validate()?;}
        // Measure actual JSON escaping while borrowed, before encoded allocation.
        struct HeaderCount(usize);impl Write for HeaderCount{
            fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{self.0=self.0.checked_add(bytes.len()).ok_or_else(||std::io::Error::other("header budget"))?;if self.0>MAX_HEADER_BYTES{return Err(std::io::Error::other("header budget"));}Ok(bytes.len())}
            fn flush(&mut self)->std::io::Result<()>{Ok(())}
        }
        serde_json::to_writer(&mut HeaderCount(0),&self.header).map_err(|e|e.to_string())?;
        let encoded = serde_json::to_vec(&self.header).map_err(|e| e.to_string())?;
        if encoded.is_empty() || encoded.len() > MAX_HEADER_BYTES {
            return Err("Oversized editor header".into());
        }
        writer
            .write_all(&(encoded.len() as u32).to_le_bytes())
            .map_err(|e| e.to_string())?;
        writer.write_all(&encoded).map_err(|e| e.to_string())?;
        writer.write_all(&self.pixels).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
// Private proposals inside rvn_core::editor_preview::tests.
// Protocol bytes only, not a real rendered/GPU frame. Not installed or run.
fn old_frame_bytes_0035(finished: Option<serde_json::Value>) -> Vec<u8> {
    let mut header = serde_json::json!({"Frame": {
        "scope": {"instance":7,"epoch":2,"sequence":3,"control_sequence":1,
            "control_ack":1,"control_error":null,"paused":false,
            "publication_pending":false,"step_allowed":false,"interaction":null,
            "position":{"pc":9,"origin":null},"active":null,"parked":[]},
        "width":1,"height":1,"debugger":null,"inspection":null
    }});
    if let Some(value) = finished {
        header["Frame"]["scope"].as_object_mut().unwrap().insert("finished".into(), value);
    }
    let encoded = serde_json::to_vec(&header).unwrap();
    let mut bytes = (encoded.len() as u32).to_le_bytes().to_vec();
    bytes.extend(encoded);
    bytes.extend([1,2,3,255]);
    bytes
}

#[test]
fn terminal_observation_0035_actual_wire_reader_keeps_legacy_unknown_and_known_boolean_separate() {
    for (field, expected) in [
        (None,None), (Some(serde_json::Value::Null),None),
        (Some(serde_json::json!(false)),Some(false)),
        (Some(serde_json::json!(true)),Some(true)),
    ] {
        let packet = Packet::read(&mut std::io::Cursor::new(old_frame_bytes_0035(field))).unwrap();
        let Header::Frame {scope,width,height,..} = &packet.header else {panic!("expected the actual Frame variant")};
        assert_eq!(scope.finished, expected);
        assert_eq!((scope.instance,scope.epoch,scope.sequence,scope.control_sequence,scope.control_ack),(7,2,3,1,1));
        assert_eq!((*width,*height),(1,1));
        assert_eq!(packet.pixels,[1,2,3,255]);
        assert!(scope.interaction.is_none() && !scope.step_allowed);
        let mut rewritten=Vec::new(); packet.write(&mut rewritten).unwrap();
        assert_eq!(Packet::read(&mut std::io::Cursor::new(rewritten)).unwrap().header,packet.header);
    }
}

#[test]
fn terminal_observation_0035_malformed_metadata_and_control_fields_remain_refused() {
    assert_eq!(WIRE_VERSION,6,"FrameConsumed credit is explicitly wire6; observations grant no command authority");
    for invalid in [serde_json::json!("true"),serde_json::json!(1),serde_json::json!([])] {
        assert!(Packet::read(&mut std::io::Cursor::new(old_frame_bytes_0035(Some(invalid)))).is_err());
    }
    let command=Header::Control {instance:7,epoch:2,sequence:4,frame_control_sequence:1,position:crate::scene::ResumePosition {pc:9,origin:None},interaction:None,kind:ControlKind::Pause};
    let mut invalid=serde_json::to_value(&command).unwrap();
    invalid["Control"].as_object_mut().unwrap().insert("finished".into(),serde_json::json!(true));
    assert!(serde_json::from_value::<Header>(invalid).is_err(),"observational metadata cannot extend strict control envelopes");
    assert_eq!(serde_json::from_value::<Header>(serde_json::to_value(&command).unwrap()).unwrap(),command);
}

    use super::*;
    #[test]
    fn bgra_wire_keeps_actual_bytes_and_rejects_false_dimensions() {
        let scope = FrameScope {
            instance: 7,
            epoch: 2,
            sequence: 3,
            control_sequence: 1,
            control_ack: 1,
            control_error: None,
            paused: true,
            publication_pending: false, finished: None,
            step_allowed: true,
            interaction: None,
            position: crate::scene::ResumePosition {
                pc: 9,
                origin: None,
            },
            active: None,
            parked: vec![],
        };
        let packet = Packet {
            header: Header::Frame {
                scope,
                width: 2,
                height: 1,
                debugger:None,
                inspection:None,
            },
            pixels: vec![1, 2, 3, 255, 4, 5, 6, 255],
        };
        let mut bytes = Vec::new();
        packet.write(&mut bytes).unwrap();
        let actual = Packet::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(actual.header, packet.header);
        assert_eq!(actual.pixels, packet.pixels);
        let mut bad = packet;
        bad.pixels.pop();
        assert!(bad.write(&mut Vec::new()).is_err());
        assert!(image_bytes(MAX_WIDTH + 1, 1).is_err());
    }
}

#[cfg(test)]
mod image_input_shape_contracts {
 use super::*;
 #[test]
 fn opaque_image_input_preserves_unicode_and_rejects_invalid_payload_bounds(){
  let value=ImageInput::Text{value:"é🌟".into(),composition:Some([0,2])};
  assert_eq!(ImageInput::decode(&value.encode().unwrap()).unwrap(),value);
  assert!(ImageInput::Text{value:"é🌟".into(),composition:Some([0,3])}.encode().is_err());
  assert!(ImageInput::Text{value:"a".repeat(8193),composition:None}.encode().is_err());
  assert!(ImageInput::decode(&" ".repeat(MAX_INPUT_BYTES+1)).is_err());
  assert!(ImageInput::Pointer{kind:PointerKind::Down,button:3,point:[1.0,1.0],wheel:[0.0;2]}.encode().is_err());
  assert!(ImageInput::Pointer{kind:PointerKind::Move,button:0,point:[f32::NAN,1.0],wheel:[0.0;2]}.encode().is_err());
  assert!(ImageInput::Key{key:"K".into(),logical:Some("two".into()),pressed:true,modifiers:[false;4]}.encode().is_err());
 }
}

#[cfg(test)]
mod release_wire_contract_0028 {
    use super::*;
    #[test]
    fn image_release_0028_has_no_positive_variant_and_keeps_original_scope_over_wire() {
        let release = ImageRelease::Pointer { button: 0, point: [12.0, 34.0], wheel: [0.0; 2] };
        let header = Header::Release { instance: 41, epoch: 7, sequence: 101, frame_sequence: 83, release: release.clone() };
        let mut bytes = Vec::new();
        Packet::header(header.clone()).write(&mut bytes).unwrap();
        assert_eq!(Packet::read(&mut bytes.as_slice()).unwrap().header, header);
        assert_eq!(release.into_input(), ImageInput::Pointer { kind: PointerKind::Up, button: 0, point: [12.0, 34.0], wheel: [0.0; 2] });
        assert!(serde_json::from_str::<ImageRelease>(r#"{"Pointer":{"kind":"Down","button":0,"point":[12,34],"wheel":[0,0]}}"#).is_err());
        assert!(serde_json::from_str::<ImageRelease>(r#"{"Key":{"key":"KeyA","logical":"a","modifiers":[false,false,false,false],"pressed":true}}"#).is_err());
        assert!(serde_json::from_str::<ImageRelease>(r#"{"Text":{"value":"activate"}}"#).is_err());
    }
    #[test]
    fn image_release_0028_bounds_apply_on_read_and_write_and_legacy_opaque_input_stays_opaque() {
        for release in [ImageRelease::Pointer { button: 3, point: [0.0; 2], wheel: [0.0; 2] },
            ImageRelease::Pointer { button: 0, point: [f32::INFINITY, 0.0], wheel: [0.0; 2] },
            ImageRelease::Key { key: "a".repeat(33), logical: None, modifiers: [false; 4] },
            ImageRelease::Key { key: "KeyA".into(), logical: Some("two".into()), modifiers: [false; 4] }] {
            assert!(release.validate().is_err());
            assert!(Packet::header(Header::Release { instance: 41, epoch: 7, sequence: 1, frame_sequence: 1, release }).write(&mut Vec::new()).is_err());
        }
        let encoded = br#"{"Release":{"instance":41,"epoch":7,"sequence":1,"frame_sequence":1,"release":{"Key":{"key":"","logical":null,"modifiers":[false,false,false,false]}}}}"#;
        let mut bytes = (encoded.len() as u32).to_le_bytes().to_vec(); bytes.extend_from_slice(encoded);
        assert!(Packet::read(&mut bytes.as_slice()).is_err());
        let original = Header::Input { instance: 41, epoch: 7, sequence: 2, frame_sequence: 1, input: "{not JSON".into() };
        let mut bytes = Vec::new(); Packet::header(original.clone()).write(&mut bytes).unwrap();
        assert_eq!(Packet::read(&mut bytes.as_slice()).unwrap().header, original);
    }
}

#[cfg(test)]
mod consumption_wire_tests_0938 {
 use super::*;
 #[test]
 fn wire6_consumption_roundtrip_is_a_header_without_control_ordinal_or_pixels(){
  assert_eq!(WIRE_VERSION,6);let header=Header::FrameConsumed{instance:7,epoch:3,frame_sequence:51};
  let mut bytes=Vec::new();Packet::header(header.clone()).write(&mut bytes).unwrap();
  let packet=Packet::read(&mut &bytes[..]).unwrap();assert_eq!(packet.header,header);assert!(packet.pixels.is_empty());
  let value=serde_json::to_value(&header).unwrap();assert!(value["FrameConsumed"].get("sequence").is_none());assert!(value["FrameConsumed"].get("control_ack").is_none());
 }
 #[test]
 fn consumption_envelope_cannot_smuggle_an_input_or_control_permit(){
  for extra in ["sequence","kind","command","input","frame_control_sequence","position"]{
   let mut value=serde_json::json!({"FrameConsumed":{"instance":7,"epoch":3,"frame_sequence":51}});
   value["FrameConsumed"][extra]=serde_json::json!(1);assert!(serde_json::from_value::<Header>(value).is_err());
  }
 }
}
