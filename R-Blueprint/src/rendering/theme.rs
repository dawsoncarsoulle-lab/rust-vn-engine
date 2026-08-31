//! Theme and styling for blueprint rendering

use bevy::prelude::*;
use crate::core::pin::PinKind;

/// Theme colors for the blueprint editor
#[derive(Debug, Clone, Resource)]
pub struct BlueprintTheme {
    /// Background color of the application
    pub background_app: Color,
    
    /// Background color of the canvas
    pub background_canvas: Color,
    
    /// Background color of panels
    pub background_panel: Color,
    
    /// Alternate background color for panels
    pub background_panel_alt: Color,
    
    /// Subtle border color
    pub border_subtle: Color,
    
    /// Secondary subtle border color
    pub border_subtle_2: Color,
    
    /// Primary text color
    pub text_primary: Color,
    
    /// Secondary text color
    pub text_secondary: Color,
    
    /// Dim text color
    pub text_dim: Color,
    
    /// Accent color
    pub accent: Color,
    
    /// Soft accent color
    pub accent_soft: Color,
    
    /// Node background (top gradient)
    pub node_bg_top: Color,
    
    /// Node background (bottom gradient)
    pub node_bg_bottom: Color,
    
    /// Node border color
    pub node_border: Color,
    
    /// Pin colors for each type
    pub pin_colors: std::collections::HashMap<PinKind, Color>,
    
    /// Pin glow colors for each type
    pub pin_glow_colors: std::collections::HashMap<PinKind, Color>,
    
    /// Node header gradients for each type
    pub node_header_gradients: std::collections::HashMap<String, (Color, Color)>,
    
    /// Grid color for canvas background
    pub grid_color: Color,
    
    /// Grid size
    pub grid_size: f32,
    
    /// Wire thickness
    pub wire_thickness: f32,
    
    /// Wire thickness for execution pins
    pub wire_thickness_exec: f32,
    
    /// Node corner radius
    pub node_corner_radius: f32,
    
    /// Node padding
    pub node_padding: f32,
    
    /// Node header height
    pub node_header_height: f32,
    
    /// Pin size
    pub pin_size: f32,
    
    /// Pin size for execution pins
    pub pin_size_exec: f32,
}

impl Default for BlueprintTheme {
    fn default() -> Self {
        let mut pin_colors = std::collections::HashMap::new();
        pin_colors.insert(PinKind::Exec, Color::rgb(0.95, 0.95, 0.95));
        pin_colors.insert(PinKind::Bool, Color::rgb(0.76, 0.27, 0.30));
        pin_colors.insert(PinKind::Float, Color::rgb(0.37, 0.82, 0.64));
        pin_colors.insert(PinKind::Int, Color::rgb(0.22, 0.78, 0.85));
        pin_colors.insert(PinKind::String, Color::rgb(0.90, 0.42, 0.41));
        pin_colors.insert(PinKind::Object, Color::rgb(0.35, 0.58, 0.96));
        pin_colors.insert(PinKind::Any, Color::rgb(0.80, 0.80, 0.80));
        
        let mut pin_glow_colors = std::collections::HashMap::new();
        pin_glow_colors.insert(PinKind::Exec, Color::rgba(1.0, 1.0, 1.0, 0.8));
        pin_glow_colors.insert(PinKind::Bool, Color::rgba(1.0, 0.4, 0.4, 0.8));
        pin_glow_colors.insert(PinKind::Float, Color::rgba(0.4, 1.0, 0.6, 0.8));
        pin_glow_colors.insert(PinKind::Int, Color::rgba(0.4, 0.8, 1.0, 0.8));
        pin_glow_colors.insert(PinKind::String, Color::rgba(1.0, 0.6, 0.8, 0.8));
        pin_glow_colors.insert(PinKind::Object, Color::rgba(0.6, 0.8, 1.0, 0.8));
        pin_glow_colors.insert(PinKind::Any, Color::rgba(0.9, 0.9, 0.9, 0.8));
        
        let mut node_header_gradients = std::collections::HashMap::new();
        node_header_gradients.insert(
            "event".to_string(),
            (Color::rgb(0.63, 0.19, 0.21), Color::rgb(0.44, 0.12, 0.15)),
        );
        node_header_gradients.insert(
            "function".to_string(),
            (Color::rgb(0.13, 0.39, 0.67), Color::rgb(0.09, 0.29, 0.53)),
        );
        node_header_gradients.insert(
            "branch".to_string(),
            (Color::rgb(0.27, 0.31, 0.38), Color::rgb(0.18, 0.22, 0.28)),
        );
        node_header_gradients.insert(
            "math".to_string(),
            (Color::rgb(0.12, 0.56, 0.47), Color::rgb(0.08, 0.41, 0.35)),
        );
        node_header_gradients.insert(
            "variable".to_string(),
            (Color::rgb(0.42, 0.58, 0.96), Color::rgb(0.28, 0.45, 0.80)),
        );
        
        Self {
            background_app: Color::hex("#131316").unwrap(),
            background_canvas: Color::hex("#18181c").unwrap(),
            background_panel: Color::hex("#1b1b20").unwrap(),
            background_panel_alt: Color::hex("#212127").unwrap(),
            border_subtle: Color::rgba(1.0, 1.0, 1.0, 0.07),
            border_subtle_2: Color::rgba(1.0, 1.0, 1.0, 0.045),
            text_primary: Color::hex("#eaeaee").unwrap(),
            text_secondary: Color::hex("#9c9ca8").unwrap(),
            text_dim: Color::hex("#63636c").unwrap(),
            accent: Color::hex("#ffb443").unwrap(),
            accent_soft: Color::rgba(1.0, 0.71, 0.27, 0.4),
            node_bg_top: Color::hex("#2b2b31").unwrap(),
            node_bg_bottom: Color::hex("#232328").unwrap(),
            node_border: Color::rgba(0.0, 0.0, 0.0, 0.6),
            pin_colors,
            pin_glow_colors,
            node_header_gradients,
            grid_color: Color::rgba(1.0, 1.0, 1.0, 0.13),
            grid_size: 112.0,
            wire_thickness: 2.4,
            wire_thickness_exec: 2.8,
            node_corner_radius: 9.0,
            node_padding: 12.0,
            node_header_height: 32.0,
            pin_size: 11.0,
            pin_size_exec: 13.0,
        }
    }
}

/// Plugin for setting up the blueprint theme
pub struct BlueprintThemePlugin;

impl Plugin for BlueprintThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BlueprintTheme>()
            .add_systems(Startup, setup_theme);
    }
}

fn setup_theme(mut theme: ResMut<BlueprintTheme>) {
    // Theme is already initialized with defaults
    // Can override specific values here if needed
}

/// Component to mark entities as part of the blueprint UI
#[derive(Debug, Clone, Component)]
pub struct BlueprintUi;

/// Component for glass effect
#[derive(Debug, Clone, Component)]
pub struct GlassEffect {
    pub intensity: f32,
    pub tint: Color,
}

impl Default for GlassEffect {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            tint: Color::rgba(0.1, 0.1, 0.15, 0.3),
        }
    }
}
