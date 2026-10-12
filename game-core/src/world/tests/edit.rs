use super::super::*;

    #[test]
    fn digging_under_water_fills_the_hole_and_leaves_the_surface_alone() {
        let top = WATER_LEVEL as i32 - 1;
        let mut w = World::generate(1);
        // Open water at least two deep, with water on every side.
        let (x, z) = (0..SIZE_Z)
            .flat_map(|z| (0..SIZE_X).map(move |x| (x as i32, z as i32)))
            .find(|&(x, z)| {
                let h = w.heights[z as usize * SIZE_X + x as usize] as i32;
                h <= top - 1
                    && !w.frozen[z as usize * SIZE_X + x as usize]
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .all(|(dx, dz)| w.is_liquid(x + dx, h, z + dz))
            })
            .expect("a deep spot");
        let bed = w.heights[z as usize * SIZE_X + x as usize] as i32 - 1;
        assert!(w.remove_block(x, bed, z).is_some());
        assert_eq!(w.get(x, bed, z), Block::Water, "the hole fills with water");
        // And you can keep digging down: each new hole is water too.
        assert!(w.remove_block(x, bed - 1, z).is_some());
        assert_eq!(w.get(x, bed - 1, z), Block::Water);
        assert_eq!(w.get(x, top, z), Block::Water, "the surface is untouched");
    }

    #[test]
    fn open_spot_is_open_and_on_the_ground() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let p = w.open_spot(SIZE_X as i32 / 2, SIZE_Z as i32 / 2 - 7);
            let (x, y, z) = (p.x.floor() as i32, p.y as i32, p.z.floor() as i32);
            assert_ne!(w.get(x, y - 1, z), Block::Air, "standing on something");
            for dy in 0..3 {
                assert_eq!(w.get(x, y + dy, z), Block::Air);
            }
        }
    }

    #[test]
    fn replace_block_only_swaps_existing_blocks() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 - 1, s.z.floor() as i32);
        assert_ne!(w.get(x, y, z), Block::Air);
        assert!(w.replace_block(x, y, z, Block::Burning));
        assert_eq!(w.get(x, y, z), Block::Burning);
        assert!(!w.replace_block(x, y + 5, z, Block::Coal), "nothing there to replace");
        assert!(!w.replace_block(x, y, z, Block::Air));
    }

    #[test]
    fn trees_grow_on_open_grass_and_not_where_blocked() {
        let mut w = World::generate(3);
        // Find a grass block with a lot of air above it (grass is on the west side).
        let mut spot = None;
        'search: for x in 6..70i32 {
            for z in 6..SIZE_Z as i32 - 6 {
                let top = w.heights[z as usize * SIZE_X + x as usize] as i32 - 1;
                let open = (1..=12).all(|dy| w.get(x, top + dy, z) == Block::Air);
                if w.get(x, top, z) == Block::Grass && open {
                    spot = Some((x, top, z));
                    break 'search;
                }
            }
        }
        let (x, y, z) = spot.expect("an open grass block");
        assert!(w.tree_fits(x, y, z, false) && w.tree_fits(x, y, z, true));

        assert!(w.grow_tree(x, y, z, false));
        assert_eq!(w.get(x, y + 1, z), Block::Log, "oak trunk on top of the grass");
        assert!((0..4).all(|dy| w.get(x, y + 1 + dy, z) == Block::Log));
        // Now the column is taken, so nothing else fits there.
        assert!(!w.grow_tree(x, y, z, true));
        assert!(!w.tree_fits(x, y, z, false));

        // A pine somewhere else is made of pine wood.
        let mut other = World::generate(3);
        assert!(other.grow_tree(x, y, z, true));
        assert_eq!(other.get(x, y + 1, z), Block::PineLog);
        assert!((0..6).all(|dy| other.get(x, y + 1 + dy, z) == Block::PineLog));
    }

    #[test]
    fn block_within_measures_distance() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 + 3, s.z.floor() as i32);
        assert!(w.place_block(x, y, z, Block::Forge));
        let center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
        assert!(w.block_within(center + Vec3::new(5.0, 0.0, 0.0), 6.0, Block::Forge));
        assert!(!w.block_within(center + Vec3::new(7.0, 0.0, 0.0), 6.0, Block::Forge));
        assert!(!w.block_within(center, 6.0, Block::HousePlant));
    }

    #[test]
    fn stairs_climb_one_block_per_step_and_need_room() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 + 4, s.z.floor() as i32);
        let cells: Vec<_> = World::stair_cells(x, y, z, (1, 0), 3).collect();
        assert_eq!(cells, vec![(x + 1, y + 1, z), (x + 2, y + 2, z), (x + 3, y + 3, z)]);
        assert!(w.stairs_fit(x, y, z, (1, 0), 3));
        // Something in the way of the second step.
        assert!(w.place_block(x + 2, y + 2, z, Block::Stone));
        assert!(!w.stairs_fit(x, y, z, (1, 0), 3));
        assert!(w.stairs_fit(x, y, z, (-1, 0), 3), "the other way is still open");
        // Off the edge of the world.
        assert!(!w.stairs_fit(SIZE_X as i32 - 2, y, z, (1, 0), 3));
    }
