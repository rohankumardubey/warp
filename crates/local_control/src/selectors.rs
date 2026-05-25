//! Serializable selectors for targeting windows, tabs, panes, and Drive objects.
use serde::{Deserialize, Serialize};

/// Opaque window identifier supplied by Warp metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowSelector(pub String);

/// Opaque tab identifier supplied by Warp metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TabSelector(pub String);

/// Opaque pane identifier supplied by Warp metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneSelector(pub String);

/// Opaque Warp Drive object identifier supplied by Warp metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DriveObjectSelector(pub String);

/// Allowlisted Drive object types supported by `warpctrl` Drive mutations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriveObjectType {
    Workflow,
    Notebook,
    EnvVarCollection,
    Prompt,
    Folder,
}

/// Hierarchical target for actions that operate on a specific Warp surface.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TargetSelector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<TabTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<PaneTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive: Option<DriveTarget>,
}

/// Window-level target selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowTarget {
    Active,
    Id { id: WindowSelector },
    Index { index: u32 },
    Title { title: String },
}

/// Tab-level target selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabTarget {
    Active,
    Id { id: TabSelector },
    Index { index: u32 },
    Title { title: String },
}

/// Pane-level target selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneTarget {
    Active,
    Id { id: PaneSelector },
    Index { index: u32 },
}

/// Warp Drive object target selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DriveTarget {
    Id {
        object_type: DriveObjectType,
        id: DriveObjectSelector,
    },
    Name {
        object_type: DriveObjectType,
        name: String,
    },
}
