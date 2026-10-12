use super::super::*;

    #[test]
    fn raycast_hits_ground_and_mining_removes_it() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        // Looking straight down from the eye hits the block we're standing on.
        let hit = w
            .raycast(s + Vec3::new(0.0, 1.62, 0.0), Vec3::NEG_Y, 5.0)
            .expect("ground within reach");
        assert_eq!(hit.1, s.y.floor() as i32 - 1);
        assert!(w.remove_block(hit.0, hit.1, hit.2).is_some());
        assert_eq!(w.get(hit.0, hit.1, hit.2), Block::Air);
        // Straight up from the eye there is nothing in reach.
        assert!(w.raycast(s + Vec3::new(0.0, 1.62, 0.0), Vec3::Y, 5.0).is_none());
        // The bottom layer can't be mined.
        assert!(w.remove_block(10, 0, 10).is_none());
    }

    #[test]
    fn raycast_face_gives_the_cell_in_front_and_blocks_can_be_placed_there() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let eye = s + Vec3::new(0.0, 1.62, 0.0);
        let hit = w.raycast_face(eye, Vec3::NEG_Y, 5.0).expect("ground");
        let ground = s.y.floor() as i32 - 1;
        assert_eq!(hit.block.1, ground);
        let place = hit.place.expect("there is air in front of the top face");
        assert_eq!(place, (hit.block.0, ground + 1, hit.block.2));

        assert!(w.place_block(place.0, place.1, place.2, Block::Stone));
        assert_eq!(w.get(place.0, place.1, place.2), Block::Stone);
        assert!(!w.place_block(place.0, place.1, place.2, Block::Dirt), "occupied");
        assert!(!w.place_block(-1, 5, 5, Block::Dirt), "outside the world");
        assert!(!w.place_block(5, SIZE_Y as i32, 5, Block::Dirt), "above the world");
        assert!(!w.place_block(5, 40, 5, Block::Air), "air is not a block");

        // The new block is what the ray hits now, with its own free cell in front of it.
        let again = w.raycast_face(eye, Vec3::NEG_Y, 5.0).unwrap();
        assert_eq!(again.block, place);
        assert_eq!(again.place, Some((place.0, place.1 + 1, place.2)));
    }
