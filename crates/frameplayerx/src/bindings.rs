//! Remappable controller bindings, saved in settings.json. Each hand's face
//! buttons, Menu/View, bumper and thumbstick press run a [`ButtonAction`],
//! and another with that hand's grip held; its thumbstick and grip +
//! thumbstick axes run an [`AxisAction`]. Pointing, the trigger, grip +
//! trigger drags, paging/scrolling menus and stepping sliders are fixed.

use serde::{Deserialize, Serialize};

pub const LEFT: usize = 0;
pub const RIGHT: usize = 1;

/// What a button press does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonAction {
    #[default]
    Nothing,
    PlayPause,
    Back,
    Library,
    PreviousVideo,
    NextVideo,
    PreviousChapter,
    NextChapter,
    SeekBack,
    SeekForward,
    BackOneMinute,
    ForwardOneMinute,
    Mute,
    Subtitles,
    Bookmark,
    ResetPicture,
    ShowHideControls,
    AdjustPanel,
    Passthrough,
    Recenter,
}

impl ButtonAction {
    pub const ALL: [ButtonAction; 20] = [
        ButtonAction::PlayPause,
        ButtonAction::Back,
        ButtonAction::Library,
        ButtonAction::PreviousVideo,
        ButtonAction::NextVideo,
        ButtonAction::PreviousChapter,
        ButtonAction::NextChapter,
        ButtonAction::SeekBack,
        ButtonAction::SeekForward,
        ButtonAction::BackOneMinute,
        ButtonAction::ForwardOneMinute,
        ButtonAction::Mute,
        ButtonAction::Subtitles,
        ButtonAction::Bookmark,
        ButtonAction::ResetPicture,
        ButtonAction::ShowHideControls,
        ButtonAction::AdjustPanel,
        ButtonAction::Passthrough,
        ButtonAction::Recenter,
        ButtonAction::Nothing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ButtonAction::Nothing => "Nothing",
            ButtonAction::PlayPause => "Play / pause",
            ButtonAction::Back => "Back",
            ButtonAction::Library => "Browse",
            ButtonAction::PreviousVideo => "Previous video",
            ButtonAction::NextVideo => "Next video",
            ButtonAction::PreviousChapter => "Previous chapter",
            ButtonAction::NextChapter => "Next chapter",
            ButtonAction::SeekBack => "Seek back",
            ButtonAction::SeekForward => "Seek forward",
            ButtonAction::BackOneMinute => "Back 1 minute",
            ButtonAction::ForwardOneMinute => "Forward 1 minute",
            ButtonAction::Mute => "Mute",
            ButtonAction::Subtitles => "Subtitles on / off",
            ButtonAction::Bookmark => "Bookmark",
            ButtonAction::ResetPicture => "Reset picture",
            ButtonAction::ShowHideControls => "Show / hide controls",
            ButtonAction::AdjustPanel => "Adjust panel",
            ButtonAction::Passthrough => "Show your room",
            ButtonAction::Recenter => "Recenter",
        }
    }

    /// A shorter label, for the controller picture.
    pub fn short_label(self) -> &'static str {
        match self {
            ButtonAction::PlayPause => "Play/pause",
            ButtonAction::PreviousVideo => "Previous",
            ButtonAction::NextVideo => "Next",
            ButtonAction::PreviousChapter => "Prev. chapter",
            ButtonAction::Subtitles => "Subtitles",
            ButtonAction::BackOneMinute => "−1 min",
            ButtonAction::ForwardOneMinute => "+1 min",
            ButtonAction::ResetPicture => "Reset view",
            ButtonAction::ShowHideControls => "Show/hide UI",
            ButtonAction::AdjustPanel => "Adjust",
            ButtonAction::Passthrough => "Room",
            other => other.label(),
        }
    }
}

/// What a thumbstick axis does. Right and up are the positive directions:
/// forward, louder, tilt up, turn right, zoom out, higher.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisAction {
    #[default]
    Nothing,
    Seek,
    Volume,
    Tilt,
    Turn,
    Zoom,
    Height,
}

impl AxisAction {
    pub const ALL: [AxisAction; 7] = [
        AxisAction::Seek,
        AxisAction::Volume,
        AxisAction::Tilt,
        AxisAction::Turn,
        AxisAction::Zoom,
        AxisAction::Height,
        AxisAction::Nothing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AxisAction::Nothing => "Nothing",
            AxisAction::Seek => "Seek",
            AxisAction::Volume => "Volume",
            AxisAction::Tilt => "Tilt the picture",
            AxisAction::Turn => "Turn the picture",
            AxisAction::Zoom => "Zoom",
            AxisAction::Height => "Raise or lower yourself",
        }
    }

    /// A shorter label, for the controller picture.
    pub fn short_label(self) -> &'static str {
        match self {
            AxisAction::Tilt => "Tilt",
            AxisAction::Turn => "Turn",
            AxisAction::Height => "Height",
            other => other.label(),
        }
    }
}

/// A remappable button, by position (see `fp_xr::Hand`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    South,
    East,
    West,
    North,
    Menu,
    Shoulder,
    StickClick,
}

impl Button {
    pub const ALL: [Button; 7] = [
        Button::South,
        Button::East,
        Button::West,
        Button::North,
        Button::Menu,
        Button::Shoulder,
        Button::StickClick,
    ];

    /// The button's name on the Frame controller of `hand`.
    pub fn label(self, hand: usize) -> &'static str {
        match (self, hand == RIGHT) {
            (Button::South, true) => "A",
            (Button::East, true) => "B",
            (Button::West, true) => "X",
            (Button::North, true) => "Y",
            (Button::Menu, true) => "Menu",
            (Button::South, false) => "D-pad down",
            (Button::East, false) => "D-pad right",
            (Button::West, false) => "D-pad left",
            (Button::North, false) => "D-pad up",
            (Button::Menu, false) => "View",
            (Button::Shoulder, _) => "Bumper",
            (Button::StickClick, _) => "Thumbstick press",
        }
    }

    pub fn pressed(self, h: &fp_xr::Hand) -> bool {
        match self {
            Button::South => h.south,
            Button::East => h.east,
            Button::West => h.west,
            Button::North => h.north,
            Button::Menu => h.menu,
            Button::Shoulder => h.shoulder,
            Button::StickClick => h.stick_click,
        }
    }
}

/// A remappable thumbstick axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    StickX,
    StickY,
    GripX,
    GripY,
}

impl Axis {
    pub fn index(self) -> usize {
        self as usize
    }
}

/// One controller's bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandBindings {
    pub south: ButtonAction,
    pub east: ButtonAction,
    pub west: ButtonAction,
    pub north: ButtonAction,
    pub menu: ButtonAction,
    pub shoulder: ButtonAction,
    pub stick_click: ButtonAction,
    pub stick_x: AxisAction,
    pub stick_y: AxisAction,
    pub grip_x: AxisAction,
    pub grip_y: AxisAction,
    pub grip: GripButtons,
}

/// What the buttons do pressed with their controller's grip held. Nothing
/// does what the button does alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GripButtons {
    pub south: ButtonAction,
    pub east: ButtonAction,
    pub west: ButtonAction,
    pub north: ButtonAction,
    pub menu: ButtonAction,
    pub shoulder: ButtonAction,
    pub stick_click: ButtonAction,
}

impl GripButtons {
    pub fn button(&self, b: Button) -> ButtonAction {
        match b {
            Button::South => self.south,
            Button::East => self.east,
            Button::West => self.west,
            Button::North => self.north,
            Button::Menu => self.menu,
            Button::Shoulder => self.shoulder,
            Button::StickClick => self.stick_click,
        }
    }

    pub fn button_mut(&mut self, b: Button) -> &mut ButtonAction {
        match b {
            Button::South => &mut self.south,
            Button::East => &mut self.east,
            Button::West => &mut self.west,
            Button::North => &mut self.north,
            Button::Menu => &mut self.menu,
            Button::Shoulder => &mut self.shoulder,
            Button::StickClick => &mut self.stick_click,
        }
    }
}

impl HandBindings {
    pub fn button(&self, b: Button) -> ButtonAction {
        match b {
            Button::South => self.south,
            Button::East => self.east,
            Button::West => self.west,
            Button::North => self.north,
            Button::Menu => self.menu,
            Button::Shoulder => self.shoulder,
            Button::StickClick => self.stick_click,
        }
    }

    pub fn button_mut(&mut self, b: Button) -> &mut ButtonAction {
        match b {
            Button::South => &mut self.south,
            Button::East => &mut self.east,
            Button::West => &mut self.west,
            Button::North => &mut self.north,
            Button::Menu => &mut self.menu,
            Button::Shoulder => &mut self.shoulder,
            Button::StickClick => &mut self.stick_click,
        }
    }

    pub fn axis(&self, a: Axis) -> AxisAction {
        match a {
            Axis::StickX => self.stick_x,
            Axis::StickY => self.stick_y,
            Axis::GripX => self.grip_x,
            Axis::GripY => self.grip_y,
        }
    }

    pub fn axis_mut(&mut self, a: Axis) -> &mut AxisAction {
        match a {
            Axis::StickX => &mut self.stick_x,
            Axis::StickY => &mut self.stick_y,
            Axis::GripX => &mut self.grip_x,
            Axis::GripY => &mut self.grip_y,
        }
    }
}

/// Both controllers. The default: the right hand runs playback, the left
/// the picture, with play/pause and seeking under either thumb; the right
/// grip makes X/Y chapters, A a bookmark and the thumbstick press subtitles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Bindings {
    pub left: HandBindings,
    pub right: HandBindings,
}

impl<'de> Deserialize<'de> for Bindings {
    /// What is saved over the defaults: a binding missing from the file
    /// (one added in a later version) comes with its default, the ones
    /// there stay as they were set.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let saved = serde_json::Value::deserialize(d)?;
        let defaults = Bindings::default();
        let hand = |name: &str, default: &HandBindings| {
            let mut v = serde_json::to_value(default).map_err(D::Error::custom)?;
            if let Some(s) = saved.get(name) {
                overlay(&mut v, s.clone());
            }
            serde_json::from_value(v).map_err(D::Error::custom)
        };
        Ok(Bindings {
            left: hand("left", &defaults.left)?,
            right: hand("right", &defaults.right)?,
        })
    }
}

/// `saved` over `base`: objects key by key, anything else replaced.
fn overlay(base: &mut serde_json::Value, saved: serde_json::Value) {
    match (base, saved) {
        (serde_json::Value::Object(b), serde_json::Value::Object(s)) => {
            for (k, v) in s {
                match b.get_mut(&k) {
                    Some(slot) => overlay(slot, v),
                    None => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, s) => *b = s,
    }
}

impl Default for Bindings {
    fn default() -> Self {
        use AxisAction as X;
        use ButtonAction as B;
        Bindings {
            left: HandBindings {
                south: B::PlayPause,
                east: B::Passthrough,
                west: B::AdjustPanel,
                north: B::ShowHideControls,
                menu: B::Recenter,
                shoulder: B::BackOneMinute,
                stick_click: B::ResetPicture,
                stick_x: X::Seek,
                stick_y: X::Tilt,
                grip_x: X::Turn,
                grip_y: X::Zoom,
                grip: GripButtons::default(),
            },
            right: HandBindings {
                south: B::PlayPause,
                east: B::Back,
                west: B::PreviousVideo,
                north: B::NextVideo,
                menu: B::Library,
                shoulder: B::ForwardOneMinute,
                stick_click: B::Mute,
                stick_x: X::Seek,
                stick_y: X::Volume,
                grip_x: X::Nothing,
                grip_y: X::Height,
                grip: GripButtons {
                    south: B::Bookmark,
                    west: B::PreviousChapter,
                    north: B::NextChapter,
                    stick_click: B::Subtitles,
                    ..GripButtons::default()
                },
            },
        }
    }
}

impl Bindings {
    pub fn hand(&self, i: usize) -> &HandBindings {
        if i == LEFT { &self.left } else { &self.right }
    }

    pub fn hand_mut(&mut self, i: usize) -> &mut HandBindings {
        if i == LEFT {
            &mut self.left
        } else {
            &mut self.right
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_and_partial_bindings_read_back() {
        let b = Bindings::default();
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("\"south\":\"play_pause\""), "{json}");
        assert_eq!(serde_json::from_str::<Bindings>(&json).unwrap(), b);
        // A file from before a binding (or a hand) existed: what it has
        // stays, what it lacks comes with its default.
        let partial: Bindings =
            serde_json::from_str(r#"{"right":{"south":"next_video","grip":{"north":"nothing"}}}"#)
                .unwrap();
        assert_eq!(partial.right.south, ButtonAction::NextVideo);
        assert_eq!(partial.right.east, ButtonAction::Back);
        let grip = &partial.right.grip;
        assert_eq!(grip.north, ButtonAction::Nothing, "set to Same as press");
        assert_eq!(grip.west, ButtonAction::PreviousChapter);
        assert_eq!(partial.left, Bindings::default().left);
        assert!(json.contains("\"grip\":{"), "{json}");
    }

    #[test]
    fn every_action_and_input_has_a_label() {
        for a in ButtonAction::ALL {
            assert!(!a.label().is_empty());
        }
        for a in AxisAction::ALL {
            assert!(!a.label().is_empty());
        }
        for b in Button::ALL {
            assert_ne!(b.label(LEFT), "");
            assert_ne!(b.label(RIGHT), "");
        }
        assert_eq!(Button::South.label(RIGHT), "A");
        assert_eq!(Button::South.label(LEFT), "D-pad down");
    }
}
