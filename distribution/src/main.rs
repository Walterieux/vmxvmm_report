use csv::Writer;
use statrs::distribution::DiscreteCDF;
use statrs::distribution::Poisson;

fn main() {
    save_plot_distribution(70.0);
}

fn save_plot_distribution(lambda: f64) {
    let n = Poisson::new(lambda).unwrap();

    let mut wtr = Writer::from_path("distribution.csv").unwrap();
    let _ = wtr.write_record(&["percentage used", "prob"]);

    for i in 0..101 {
        let _ = wtr.write_record(&[i.to_string(), n.sf(i).to_string()]);
    }

    let _ = wtr.flush();
}
