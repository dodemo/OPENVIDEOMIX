//! Live Title and Lower-Third graphics model.
//!
//! Provides typed structures for broadcast overlays, lower-thirds,
//! and tickers with spatial positioning, typography, colors,
//! background boxes, and drop shadows.

use serde::{Deserialize, Serialize};

/// Broadcast title and lower-third data model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleData {
    pub headline: String,
    pub subtitle: String,
    /// X anchor position as percentage of frame width (0.0% to 100.0%)
    pub x_percent: f32,
    /// Y anchor position as percentage of frame height (0.0% to 100.0%)
    pub y_percent: f32,
    /// Scale factor (0.5 to 2.5)
    pub scale: f32,
    pub font_size_headline: f32,
    pub font_size_subtitle: f32,
    /// RGBA text color
    pub text_color: [u8; 4],
    /// RGBA subtitle text color
    pub subtitle_color: [u8; 4],
    /// RGBA background bounding box fill
    pub bg_color: [u8; 4],
    /// RGBA left accent bar color
    pub accent_bar_color: [u8; 4],
    /// Padding around text inside the box in points
    pub bg_padding: f32,
    /// Corner radius of the bounding box
    pub corner_radius: f32,
    /// Whether drop shadow is rendered
    pub has_drop_shadow: bool,
    /// Drop shadow offset [x, y] in points
    pub shadow_offset: [f32; 2],
    /// RGBA drop shadow color
    pub shadow_color: [u8; 4],
}

impl Default for TitleData {
    fn default() -> Self {
        Self {
            headline: "OPENVIDEOMIX".into(),
            subtitle: "Emisión en Directo · 1080p50".into(),
            x_percent: 8.0,
            y_percent: 78.0,
            scale: 1.0,
            font_size_headline: 24.0,
            font_size_subtitle: 15.0,
            text_color: [255, 255, 255, 255],
            subtitle_color: [210, 220, 230, 255],
            bg_color: [18, 22, 30, 230],
            accent_bar_color: [0, 190, 255, 255],
            bg_padding: 10.0,
            corner_radius: 4.0,
            has_drop_shadow: true,
            shadow_offset: [2.0, 2.0],
            shadow_color: [0, 0, 0, 180],
        }
    }
}

impl TitleData {
    pub fn new(headline: impl Into<String>, subtitle: impl Into<String>) -> Self {
        Self {
            headline: headline.into(),
            subtitle: subtitle.into(),
            ..Default::default()
        }
    }

    /// Lower-third preset suitable for standard guest/speaker introduction
    pub fn preset_speaker(speaker: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            headline: speaker.into(),
            subtitle: role.into(),
            x_percent: 8.0,
            y_percent: 80.0,
            scale: 1.0,
            font_size_headline: 24.0,
            font_size_subtitle: 14.0,
            text_color: [255, 255, 255, 255],
            subtitle_color: [200, 215, 230, 255],
            bg_color: [15, 20, 28, 235],
            accent_bar_color: [0, 180, 240, 255],
            bg_padding: 10.0,
            corner_radius: 4.0,
            has_drop_shadow: true,
            shadow_offset: [2.0, 2.0],
            shadow_color: [0, 0, 0, 180],
        }
    }

    /// Breaking news / alert preset with high-visibility red accent
    pub fn preset_breaking_news(headline: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            headline: headline.into(),
            subtitle: details.into(),
            x_percent: 5.0,
            y_percent: 82.0,
            scale: 1.0,
            font_size_headline: 26.0,
            font_size_subtitle: 15.0,
            text_color: [255, 255, 255, 255],
            subtitle_color: [255, 230, 230, 255],
            bg_color: [25, 10, 12, 245],
            accent_bar_color: [240, 40, 40, 255],
            bg_padding: 12.0,
            corner_radius: 2.0,
            has_drop_shadow: true,
            shadow_offset: [3.0, 3.0],
            shadow_color: [0, 0, 0, 200],
        }
    }

    /// Minimal elegant lower-third preset with subtle dark translucency
    pub fn preset_minimal(title: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            headline: title.into(),
            subtitle: note.into(),
            x_percent: 8.0,
            y_percent: 82.0,
            scale: 0.95,
            font_size_headline: 22.0,
            font_size_subtitle: 13.0,
            text_color: [255, 255, 255, 255],
            subtitle_color: [190, 195, 205, 255],
            bg_color: [20, 20, 25, 180],
            accent_bar_color: [255, 255, 255, 160],
            bg_padding: 8.0,
            corner_radius: 6.0,
            has_drop_shadow: true,
            shadow_offset: [2.0, 2.0],
            shadow_color: [0, 0, 0, 140],
        }
    }

    /// Validate coordinates and scale
    pub fn sanitize(&mut self) {
        self.x_percent = self.x_percent.clamp(0.0, 95.0);
        self.y_percent = self.y_percent.clamp(0.0, 95.0);
        self.scale = self.scale.clamp(0.3, 3.0);
        self.font_size_headline = self.font_size_headline.clamp(8.0, 72.0);
        self.font_size_subtitle = self.font_size_subtitle.clamp(6.0, 48.0);
        self.bg_padding = self.bg_padding.clamp(0.0, 50.0);
        self.corner_radius = self.corner_radius.clamp(0.0, 30.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_data_default_and_presets() {
        let title = TitleData::default();
        assert_eq!(title.headline, "OPENVIDEOMIX");
        assert!(title.has_drop_shadow);

        let speaker = TitleData::preset_speaker("Dra. Carme Blanco", "Directora de Informativos");
        assert_eq!(speaker.headline, "Dra. Carme Blanco");
        assert_eq!(speaker.subtitle, "Directora de Informativos");
        assert_eq!(speaker.accent_bar_color, [0, 180, 240, 255]);

        let breaking = TitleData::preset_breaking_news("ÚLTIMA HORA", "Conexión en directo");
        assert_eq!(breaking.headline, "ÚLTIMA HORA");
        assert_eq!(breaking.accent_bar_color, [240, 40, 40, 255]);

        let minimal = TitleData::preset_minimal("Santiago de Compostela", "15:00 UTC");
        assert_eq!(minimal.headline, "Santiago de Compostela");
    }

    #[test]
    fn test_title_data_serialization_roundtrip() {
        let title = TitleData::new(
            "Titular con ñ e tiles: á, é, í, ó, ú",
            "Subtítulo en Galego",
        );
        let json = serde_json::to_string(&title).unwrap();
        let decoded: TitleData = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, title);
    }

    #[test]
    fn test_title_data_sanitize() {
        let mut title = TitleData {
            x_percent: 150.0,
            y_percent: -20.0,
            scale: 10.0,
            font_size_headline: 200.0,
            font_size_subtitle: 1.0,
            ..Default::default()
        };
        title.sanitize();
        assert_eq!(title.x_percent, 95.0);
        assert_eq!(title.y_percent, 0.0);
        assert_eq!(title.scale, 3.0);
        assert_eq!(title.font_size_headline, 72.0);
        assert_eq!(title.font_size_subtitle, 6.0);
    }
}
