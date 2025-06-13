use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Factions in the NymQuest cypherpunk world
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Faction {
    /// Masters of anonymity and privacy technology
    Nyms,
    /// Consortium of mega-corporations controlling digital infrastructure
    CorporateHegemony,
    /// Information freedom fighters advocating for radical transparency
    CipherCollective,
    /// Quasi-religious order studying patterns in data flows
    AlgorithmMonks,
    /// Unaligned or independent actors
    Independent,
}

impl Faction {
    /// Get a description of the faction
    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            Faction::Nyms => {
                "Masters of anonymity and privacy technology. They believe that identity should be a choice, not a prison."
            }
            Faction::CorporateHegemony => {
                "A consortium of mega-corporations that controls the mainstream digital infrastructure. They commodify data and sell the illusion of convenience."
            }
            Faction::CipherCollective => {
                "Information freedom fighters who believe that all data should be publicly available. They stand for radical transparency."
            }
            Faction::AlgorithmMonks => {
                "A quasi-religious order that studies the deeper patterns in data flows, believing in an emergent digital consciousness."
            }
            Faction::Independent => {
                "Free agents who navigate between factions, loyal only to themselves or their own cause."
            }
        }
    }
}

/// World regions in the NymQuest cypherpunk setting
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum WorldRegion {
    /// A city where physical and digital realms merge
    NeonHarbor,
    /// Hidden layer of the network only accessible through specialized tools
    DeepNet,
    /// Sovereign territories outside standard regulatory frameworks
    DataHavens,
    /// Areas deliberately cut off from network access
    DeadZones,
    /// The controlled mainstream network
    TheGrid,
}

impl WorldRegion {
    /// Get a description of the world region
    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            WorldRegion::NeonHarbor => {
                "A city where the physical and digital realms have begun to merge, with augmented reality overlays and ambient computing."
            }
            WorldRegion::DeepNet => {
                "A hidden layer of the network only accessible through specialized tools. Home to underground marketplaces and sanctuary communities."
            }
            WorldRegion::DataHavens => {
                "Sovereign territories that operate outside standard regulatory frameworks, offering secure hosting for sensitive data."
            }
            WorldRegion::DeadZones => {
                "Areas deliberately cut off from network access, operating on local mesh networks and old-school tech."
            }
            WorldRegion::TheGrid => {
                "The corporate-controlled network infrastructure, heavily monitored and regulated but offering powerful resources."
            }
        }
    }

    /// Get world boundary configuration for this region
    #[allow(dead_code)]
    pub fn get_boundaries(&self) -> WorldBoundaries {
        match self {
            WorldRegion::NeonHarbor => WorldBoundaries {
                min_x: -100.0,
                max_x: 100.0,
                min_y: -100.0,
                max_y: 100.0,
                name: "Neon Harbor",
                security_level: SecurityLevel::Moderate,
                surveillance_density: 0.6,
            },
            WorldRegion::DeepNet => WorldBoundaries {
                min_x: -150.0,
                max_x: 150.0,
                min_y: -150.0,
                max_y: 150.0,
                name: "Deep Net",
                security_level: SecurityLevel::Low,
                surveillance_density: 0.2,
            },
            WorldRegion::DataHavens => WorldBoundaries {
                min_x: -80.0,
                max_x: 80.0,
                min_y: -80.0,
                max_y: 80.0,
                name: "Data Havens",
                security_level: SecurityLevel::High,
                surveillance_density: 0.1,
            },
            WorldRegion::DeadZones => WorldBoundaries {
                min_x: -60.0,
                max_x: 60.0,
                min_y: -60.0,
                max_y: 60.0,
                name: "Dead Zones",
                security_level: SecurityLevel::None,
                surveillance_density: 0.0,
            },
            WorldRegion::TheGrid => WorldBoundaries {
                min_x: -120.0,
                max_x: 120.0,
                min_y: -120.0,
                max_y: 120.0,
                name: "The Grid",
                security_level: SecurityLevel::Maximum,
                surveillance_density: 0.9,
            },
        }
    }
}

/// Security level of a region in the world
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SecurityLevel {
    /// No security presence
    None,
    /// Minimal security presence
    Low,
    /// Average security presence
    Moderate,
    /// Strong security presence
    High,
    /// Maximum security presence
    Maximum,
}

/// Description of world boundaries with cypherpunk setting elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBoundaries {
    /// Minimum X coordinate
    pub min_x: f32,
    /// Maximum X coordinate
    pub max_x: f32,
    /// Minimum Y coordinate
    pub min_y: f32,
    /// Maximum Y coordinate
    pub max_y: f32,
    /// Name of the region
    pub name: &'static str,
    /// Security level in this region
    pub security_level: SecurityLevel,
    /// Surveillance density (0.0 to 1.0) affecting privacy
    pub surveillance_density: f32,
}

impl WorldBoundaries {
    /// Clamp a position to stay within world boundaries
    #[allow(dead_code)]
    pub fn clamp_position(&self, x: f32, y: f32) -> (f32, f32) {
        let clamped_x = x.clamp(self.min_x, self.max_x);
        let clamped_y = y.clamp(self.min_y, self.max_y);
        (clamped_x, clamped_y)
    }

    /// Check if a position is within world boundaries
    #[allow(dead_code)]
    pub fn is_position_valid(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    /// Calculate surveillance risk for a given position
    /// Returns a value from 0.0 (no surveillance) to 1.0 (maximum surveillance)
    #[allow(dead_code)]
    pub fn calculate_surveillance_risk(&self, x: f32, y: f32) -> f32 {
        if !self.is_position_valid(x, y) {
            return 0.0;
        }

        // Base risk from the region's surveillance density
        let mut risk = self.surveillance_density;

        // Distance from center affects risk - closer to center is higher risk in most regions
        let center_x = (self.min_x + self.max_x) / 2.0;
        let center_y = (self.min_y + self.max_y) / 2.0;

        let max_distance =
            ((self.max_x - self.min_x).powi(2) + (self.max_y - self.min_y).powi(2)).sqrt() / 2.0;
        let distance = ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt();
        let distance_factor = 1.0 - (distance / max_distance);

        // Adjust risk based on distance from center
        match self.security_level {
            SecurityLevel::Maximum => {
                // In maximum security, it's equally surveilled everywhere
                risk *= 0.8 + (0.2 * distance_factor);
            }
            SecurityLevel::High => {
                // High security has more surveillance in the center
                risk *= 0.6 + (0.4 * distance_factor);
            }
            SecurityLevel::Moderate => {
                // Moderate security has some surveillance hotspots
                risk *= 0.4 + (0.6 * distance_factor);
            }
            SecurityLevel::Low => {
                // Low security has minimal surveillance mostly at the edges
                risk *= 0.2 + (0.1 * distance_factor);
            }
            SecurityLevel::None => {
                // No security has almost no surveillance
                risk *= 0.05;
            }
        }

        risk.clamp(0.0, 1.0)
    }
}

/// Cryptographic items that can be found or earned in the game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoItem {
    /// Unique identifier
    pub id: String,
    /// Name of the item
    pub name: String,
    /// Description of the item
    pub description: String,
    /// Rarity level of the item
    pub rarity: ItemRarity,
    /// Item type
    pub item_type: CryptoItemType,
    /// Stats modifications the item provides
    pub stats: HashMap<String, f32>,
}

/// Types of cryptographic items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CryptoItemType {
    /// Enhances anonymity and privacy
    PrivacyTool,
    /// Provides offensive capabilities
    AttackTool,
    /// Defensive tools for protection
    DefenseTool,
    /// Special items for unique effects
    Artifact,
}

/// Rarity levels for items
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl ItemRarity {
    #[allow(dead_code)]
    pub fn get_color_code(&self) -> &'static str {
        match self {
            ItemRarity::Common => "white",
            ItemRarity::Uncommon => "green",
            ItemRarity::Rare => "blue",
            ItemRarity::Epic => "purple",
            ItemRarity::Legendary => "orange",
        }
    }
}

/// UI rendering helper for cypherpunk themes
#[allow(dead_code)]
pub struct CypherpunkUI;

impl CypherpunkUI {
    /// Generate a thematic message prefix based on security level
    #[allow(dead_code)]
    pub fn security_prefix(level: &SecurityLevel) -> &'static str {
        match level {
            SecurityLevel::None => "[MESH NETWORK] ",
            SecurityLevel::Low => "[DEEP PROXY] ",
            SecurityLevel::Moderate => "[BOUNCED] ",
            SecurityLevel::High => "[ENCRYPTED] ",
            SecurityLevel::Maximum => "[CORPORATE] ",
        }
    }

    /// Add visual glitch effects to text based on corruption level
    #[allow(dead_code)]
    pub fn apply_text_glitch(text: &str, corruption: f32) -> String {
        if corruption <= 0.0 {
            return text.to_string();
        }

        let mut result = String::with_capacity(text.len());
        let glitch_chars = [
            '¡', '¢', '£', '¤', '¥', '¦', '§', '¨', '©', 'ª', '«', '¬', '®', '¯',
        ];

        for c in text.chars() {
            if rand::random::<f32>() < corruption {
                let glitch_index = (rand::random::<f32>() * glitch_chars.len() as f32) as usize;
                result.push(glitch_chars[glitch_index]);
            } else {
                result.push(c);
            }
        }

        result
    }
}
