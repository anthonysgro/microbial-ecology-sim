use emergent_sovereignty::grid::config::{CellDefaults, GridConfig};
use emergent_sovereignty::grid::tick::TickOrchestrator;
use emergent_sovereignty::grid::Grid;

fn main() {
    let config = GridConfig {
        width: 10,
        height: 10,
        num_chemicals: 1,
        diffusion_rate: 0.05,
        thermal_conductivity: 0.05,
        evaporation_coefficient: 0.01,
        ambient_heat: 0.0,
        tick_duration: 1.0,
        num_threads: 4,
    };

    let defaults = CellDefaults {
        chemical_concentrations: vec![0.0],
        heat: 0.0,
        moisture: 1.0,
    };

    let mut grid = Grid::new(config.clone(), defaults).expect("valid config");

    // Seed a chemical hot spot in the center
    let center = grid.index(5, 5).expect("in bounds");
    grid.write_chemical(0).expect("species 0")[center] = 100.0;
    grid.swap_chemicals();

    // Seed a heat source in the center
    grid.write_heat()[center] = 50.0;
    grid.swap_heat();

    println!("=== Environment Grid Demo ===");
    println!("Grid: {}x{}, 1 chemical species", config.width, config.height);
    println!("Center cell (5,5) seeded: chemical=100.0, heat=50.0, moisture=1.0\n");

    for tick in 0..10 {
        if let Err(e) = TickOrchestrator::step(&mut grid, &config) {
            eprintln!("Tick {} failed: {:?}", tick, e);
            return;
        }

        let ci = grid.index(5, 5).expect("in bounds");
        let chem = grid.read_chemical(0).expect("species 0")[ci];
        let heat = grid.read_heat()[ci];
        let moisture = grid.read_moisture()[ci];

        let total_chem: f32 = grid.read_chemical(0).expect("species 0").iter().sum();
        let total_heat: f32 = grid.read_heat().iter().sum();
        let total_moisture: f32 = grid.read_moisture().iter().sum();

        println!(
            "Tick {:2} | center: chem={:.4} heat={:.4} moist={:.4} | totals: chem={:.2} heat={:.2} moist={:.4}",
            tick + 1, chem, heat, moisture, total_chem, total_heat, total_moisture
        );
    }

    // Print the chemical field as a 2D heatmap
    println!("\n--- Chemical concentration after 10 ticks ---");
    let chem_buf = grid.read_chemical(0).expect("species 0");
    for y in 0..config.height {
        let row: Vec<String> = (0..config.width)
            .map(|x| {
                let idx = (y as usize) * (config.width as usize) + (x as usize);
                let v = chem_buf[idx];
                if v < 0.01 { "  .  ".to_string() } else { format!("{:5.1}", v) }
            })
            .collect();
        println!("{}", row.join(""));
    }
}
