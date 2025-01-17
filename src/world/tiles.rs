use std::cmp::Ordering;
use serde::{Deserialize, Serialize};
use crate::mul::tiledata::{LandTileData, MulTileFlags, StaticTileData};

/// stores information about dynamic objects in the world
/// can now have two options for items:
/// GameObject - any game items
/// MultiPart part of a multi-object, usually a house
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum DynamicWorldObject {
    MultiPart{
        x: isize,
        y: isize,
        z: i8,
        tile: u16,
        parent: u32,
        counter: u16,
    },

    GameObject{
        x: isize,
        y: isize,
        z: i8,
        serial: u32,
        graphic: u16,
    }
}

impl DynamicWorldObject {
    /// return the minimum item with the given coordinates
    pub fn min_item(x: isize, y: isize) -> Self {
        // MultiPart is less than GameObject
        DynamicWorldObject::MultiPart {
            x, y, z: i8::MIN,
            tile: u16::MIN,
            parent: u32::MIN,
            counter: u16::MIN,
        }
    }

    /// return the minimum item with the given coordinates
    pub fn max_item(x: isize, y: isize) -> Self {
        // GameObject is greater that MultiPart
        DynamicWorldObject::GameObject {
            x, y, z: i8::MAX,
            serial: u32::MAX,
            graphic: u16::MAX,
        }
    }
}

impl PartialOrd for DynamicWorldObject {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DynamicWorldObject {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // identical enums are compared as tuples
            (Self::GameObject { x: x1, y: y1, z: z1, serial: serial1, graphic: graphic1 },
             Self::GameObject { x: x2, y: y2, z: z2, serial: serial2, graphic: graphic2 }) =>
                (x1, y1, z1, serial1, graphic1).cmp(&(x2, y2, z2, serial2, graphic2)),

            (Self::MultiPart { x: x1, y: y1, z: z1, tile: tile1, parent: parent1, counter: counter1 },
             Self::MultiPart { x: x2, y: y2, z: z2, tile: tile2, parent: parent2, counter: counter2 }) =>
                    (x1, y1, z1, tile1, parent1, counter1).cmp(&(x2, y2, z2, tile2, parent2, counter2)),

            (Self::MultiPart { x: x1, y: y1, z: z1, .. },
             Self::GameObject { x: x2, y: y2, z: z2, .. }) |
            (Self::GameObject { x: x1, y: y1, z: z1, .. },
             Self::MultiPart { x: x2, y: y2, z: z2, .. }) => {
                // to compare different enums, we compare the coordinates
                let order = (x1, y1, z1).cmp(&(x2, y2, z2));
                if order != Ordering::Equal {
                    return order
                }

                // and if the coordinates are equal, then we consider that the MultiPart is less than the GameObject
                match (self, other) {
                    (Self::MultiPart {..}, Self::GameObject {..}) => Ordering::Less,
                    (Self::GameObject {..}, Self::MultiPart {..}) => Ordering::Greater,
                    _ => unreachable!()
                }
            }
        }
    }
}

#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum TileType {
    MapTile(u16),
    ObjectTile(u16),
}

impl TileType {
    #[inline(always)]
    pub fn num(&self) -> u16 {
        match self {
            TileType::MapTile(tile) |
            TileType::ObjectTile(tile) => *tile
        }
    }
}

#[derive(Copy, Clone)]
pub enum TileShape {
    Slope {z_base: i8, z_stand:i8, z_top: i8, passable: bool, },
    Surface {z_base: i8, z_stand: i8, passable: bool,},
    Background {z_base: i8, z_top: i8, },
}


impl TileShape {
    pub fn from_static_tile(z: i8, static_tile: &StaticTileData) -> Self {
        let passable = static_tile.flags & MulTileFlags::Impassable as u32 == 0;

        let z_base = z;
        let height = static_tile.height as i8;
        let z_top = z_base.saturating_add(height);  // Might be worth switching to 16bit z, at least internally?

        if static_tile.flags & (MulTileFlags::Impassable as u32 | MulTileFlags::Surface as u32) == 0 {
            return Self::background(z_base, z_top)
        };

        // TODO remove hack to walk through doors
        if static_tile.flags & MulTileFlags::Door as u32 != 0 {
            return Self::background(z_base, z_top)
        };

        if static_tile.flags & MulTileFlags::Bridge as u32 == 0 {
            let z_stand = z_top;
            Self::flat(z_base, z_stand, passable)
        } else {
            let z_stand = z_base + height / 2;
            Self::slope(z_base, z_stand, z_top, passable)
        }
    }

    pub fn from_land_tile(z_base: i8, z_stand: i8, z_top: i8, tile: u16, land_tile: &LandTileData) -> Self {
        let passable = land_tile.flags & MulTileFlags::Impassable as u32 == 0;

        if tile == 0x0002 || tile == 0x01DB || (tile >= 0x01AE && tile <= 0x01B5)  {
            return Self::background(z_base, z_top);
        }

        if z_base == z_stand && z_stand == z_top {
            Self::flat(z_base, z_stand, passable)
        } else {
            Self::slope(z_base, z_stand, z_top, passable)
        }
    }

    #[inline]
    pub fn flat(z_base: i8, z_stand: i8, passable: bool) -> Self {
        Self::Surface { z_base, z_stand, passable }
    }

    #[inline]
    pub fn slope(z_base: i8, z_stand: i8, z_top: i8, passable: bool) -> Self {
        Self::Slope { z_base, z_stand, z_top, passable }
    }

    #[inline]
    pub fn background(z_base: i8, z_top: i8) -> Self{
        Self::Background { z_base, z_top, }
    }

}

/// base representation of the tile, stores information about the type of tile and its number
/// and also stores information that is used when checking the movement
#[derive(Copy, Clone)]
pub struct WorldTile {
    pub tile: TileType,
    pub shape: TileShape,
}


impl WorldTile {
    ///  return a special tile that is added as the topmost element during walkability checks
    pub fn cap_tile() -> Self {
        Self{
            tile: TileType::MapTile(0),
            shape: TileShape::flat(i8::MAX, i8::MAX, false),
        }
    }

    /// returns the z-coordinate of the base of the tile
    #[inline]
    pub fn z_base(&self) -> i8 {
        match self.shape {
            TileShape::Slope { z_base, .. } |
            TileShape::Surface { z_base, .. } |
            TileShape::Background { z_base, .. } => z_base
        }
    }

    /// returns the z coordinate where the character will stand on the given tile
    /// VERY IMPORTANT: if you call the function with the wrong tile, it will lead to a panic!
    #[inline]
    pub fn z_stand(&self) -> i8 {
        match self.shape {
            TileShape::Slope { z_stand, passable: true, .. } => z_stand,
            TileShape::Surface { z_stand, passable: true, .. } => z_stand,
            _ => panic!("z_stand called for invalid tile type"),
        }
    }

    /// returns the z-coordinate of the top of the tile
    #[inline]
    pub fn z_top(&self) -> i8 {
        match self.shape {
            TileShape::Slope { z_top, .. } |
            TileShape::Surface { z_stand: z_top, .. } |
            TileShape::Background { z_top, .. } => z_top
        }
    }

    #[inline]
    pub fn is_slope(&self) -> bool {
        match self.shape {
            TileShape::Slope { .. } => true,
            _ => false,
        }
    }

    #[inline]
    pub fn is_land(&self) -> bool {
        match self.tile {
            TileType::MapTile(_) => true,
            _ => false,
        }
    }
}

/// The game object representation used for indexing is a "double"/"reflection" of the DynamicWorldObject
#[derive(Serialize, Deserialize)]
pub struct TopLevelItem {
    pub world: u8,
    pub x: isize,
    pub y: isize,
    pub z: i8,
    pub serial: u32,
    pub graphic: u16,
    #[serde(default)]
    pub last_updated: u64,
}
