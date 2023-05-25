use std::{process, fs, io::Write};

use gtk4::gio::Cancellable;
use gtk4::glib::MainContext;
use hound::WavReader;
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit, FrequencySpectrum};
use gtk4::{prelude::*, Button, glib};
use gtk4::{Application, ApplicationWindow};

const APP_ID: &str = "com.github.jmir1.vbtools";

fn main() {
    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    let search_fr = 3400.0;

    // Connect to "activate" signal of `app`
    app.connect_activate(build_ui);
    // Run the application
    app.run();

    // convert input file to wav:
    

    // ffmpeg -i input.mp4 -vn -acodec pcm_s16le -ar 44100 -ac 1 output.wav
    // Specify the path to your audio file
    let audio_file = "output.wav";

    // Read the audio file using the hound crate
    println!("Reading audio file...");
    let mut reader: WavReader<std::io::BufReader<std::fs::File>> =
        WavReader::open(audio_file).unwrap();
    let samples = reader
        .samples::<i16>()
        .map(|x| x.unwrap() as f32)
        .collect::<Vec<f32>>();
    let samples_chunked = samples.chunks(2048).collect::<Vec<&[f32]>>();

    // apply hann window for smoothing; length must be a power of 2 for the FFT
    // 2048 is a good starting point with 44100 Hz
    println!("Calculating spectrum...");
    let mut whistles: Vec<(f32, f32)> = Vec::new();
    for i in 0..samples_chunked.len() - 1 {
        let time = (i as f32) * 2048.0 / 44100.0;
        let hann_window = hann_window(samples_chunked[i]);
        // calc spectrum
        let spectrum: FrequencySpectrum = samples_fft_to_spectrum(
            // (windowed) samples
            &hann_window,
            // sampling rate
            44100,
            // optional frequency limit: e.g. only interested in frequencies 50 <= f <= 150?
            FrequencyLimit::Range(search_fr - 100.0, search_fr + 100.0),
            // optional scale
            Some(&divide_by_N_sqrt),
        )
        .unwrap();
        let mut devsum = 0.0;
        for (f, v) in spectrum.data() {
            let frequency = f.val();
            let value = v.val();
            let dev = (search_fr - frequency).abs().powi(2) / value;
            devsum += dev;
        }
        let last_whistle = whistles.last().unwrap_or(&(-2.0, -1.0));
        if devsum < 5.0 {
            if time - last_whistle.1 > 0.5 {
                whistles.push((time, time))
            } else {
                let start = whistles.pop().unwrap().0;
                whistles.push((start, time));
            }
        }
    }
    // filter out short whistles
    whistles = whistles.into_iter().filter(|x| x.1 - x.0 > 0.1).collect();
    if whistles.len() % 2 == 1 {
        whistles.pop();
    }
    let rallyes = whistles
        .chunks(2)
        .map(|x| (x[0].1, x[1].1))
        .collect::<Vec<(f32, f32)>>();

    let mut i = 0;
    for (start, end) in &rallyes {
        println!("Rally {} from {} to {}", i, start, end);
        process::Command::new("ffmpeg")
            .args(&[
                "-ss",
                &(start - 9.0).to_string(),
                "-i",
                "video_path",
                "-ss",
                "10.0",
                "-t",
                &(end - start - 1.0).to_string(),
                "-y",
                "-nostats",
                "-loglevel",
                "0",
                &format!("rally{i}.mp4"),
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
        i += 1;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open("list.txt")
        .unwrap();

    for i in 0..rallyes.len() {
        writeln!(file, "file rally{i}.mp4").unwrap();
    }
    process::Command::new("ffmpeg")
            .args(&[
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
                "list.txt",
                "-c",
                "copy",
                "-y",
                "-nostats",
                "-loglevel",
                "0",
                &format!("output.mp4"),
            ])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
        fs::remove_file("list.txt").unwrap();
        fs::remove_file("output.wav").unwrap();
        for i in 0..rallyes.len() {
            fs::remove_file(format!("rally{i}.mp4")).unwrap();
        }
}

fn build_ui(app: &Application) {
    // Create a button with label and margins
    let button = Button::builder()
        .label("convert audio!")
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // Create a window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("My GTK App")
        .child(&button)
        .build();

    // Connect to "clicked" signal of `button`
    button.connect_clicked(move |button| {
        button.set_label("converting audio...");
        button.set_sensitive(false);
        
        let main_context = MainContext::default();
        // The main loop executes the asynchronous block
        main_context.spawn_local(glib::clone!(@weak button => async move {
            // Deactivate the bxutton until the operation is done
            button.set_sensitive(false);
            gtk4::FileDialog::new().open(
                None::<&ApplicationWindow>,
                None::<&Cancellable>,
                |result| {
                    let path = result.unwrap().path().unwrap().to_str().unwrap().to_owned();
                    MainContext::default().spawn_local(async move {
                        convert_audio(&path);
                    });
                    
                },
            );
            // Activate the button again
            button.set_sensitive(true);
        }));
    });

    // Present window
    window.present();
}

fn convert_audio(video_path: &str) {
    process::Command::new("ffmpeg")
        .args(&[
            "-i",
            video_path,
            "-vn",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "44100",
            "-ac",
            "1",
            "-y",
            "-nostats",
            "-loglevel",
            "0",
            "output.wav",
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}
