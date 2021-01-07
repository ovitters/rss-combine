use std::path::PathBuf;
use structopt::StructOpt;


#[derive(Debug, StructOpt)]
#[structopt(name = "rss-combine", about = "Merge entries from multiple rss files.")]
struct Opt {
    /// Maximum number of entries in the RSS; use 0 for unlimited entries
    #[structopt(short = "l", default_value = "0")]
    max_entries: usize,

    /// Print more details to stdout
    #[structopt(short, long)]
    verbose: bool,

    /// Main RSS file
    #[structopt(parse(from_os_str))]
    input: PathBuf,

    /// Additional files
    #[structopt(parse(from_os_str), required = true)]
    files: Vec<PathBuf>,

}

fn run_app() -> Result<(), ()> {
    let opt = Opt::from_args();

    use std::fs::File;
    use std::io::BufReader;
    use rss::Channel;
    use std::collections::HashSet;

    // Keep a list of known GUIDs to prevent duplicate RSS entries
    let mut known_guids = HashSet::new();

    if opt.verbose {
        println!("Reading original RSS: {}", &opt.input.display())
    }

    let file = File::open(&opt.input).expect("Cannot read main RSS file");
    let mut channel = Channel::read_from(BufReader::new(file)).expect("Cannot read main RSS file");

    // Keep track of the number of RSS entries without a GUID, this to warn the user as the GUID is
    // used to merge the entries
    //
    // It can indicate a problem with the RSS feed
    let mut nr_missing_guids = 0;

    // Two lists:
    // a) list of original RSS entries
    // b) list of new RSS entries
    let mut items_orig = channel.items_mut().to_vec();
    let mut items_extra = Vec::new();

    for item in items_orig.iter() {
        // This logic will remove any RSS items without an GUID
        if let Some(guid) = item.guid() {
            known_guids.insert(guid.value().to_string());
        } else {
            nr_missing_guids += 1;
        }
    }

    for rss_filename in opt.files {
        if opt.verbose {
            println!("Reading additional RSS: {}", rss_filename.display());
        }
        let file2 = match File::open(&rss_filename) {
            Ok(file2) => file2,
            Err(error) => {
                eprintln!("WARNING: Skipping unreadable RSS file {}: {}", rss_filename.display(), error);
                continue
            }
        };
        // The channel variable is reused so that the merged RSS contains the fields from the
        // newest RSS file
        channel = match Channel::read_from(BufReader::new(file2)) {
            Ok(channel) => channel,
            Err(error) => {
                eprintln!("WARNING: Skipping unparseble RSS file {}: {}", rss_filename.display(), error);
                continue
            }
        };

        /*
        // Update last build date
        // Obsolete due to reuse of channel variable
        if let Some(date) = addchannel.last_build_date() {
            channel.set_last_build_date(date.to_string());
        } */

        let mut vec_items = channel.items_mut().to_vec();

        let mut i = 0;
        while i != vec_items.len() {
            let guid = match vec_items[i].guid() {
                Some(guid) => guid,
                None       => {
                    nr_missing_guids += 1;
                    i +=1;
                    continue;
                }
            };

            if known_guids.contains(guid.value()) {
                i += 1;
                continue
            }

            known_guids.insert(guid.value().to_string());
            items_extra.push(vec_items.remove(i));
        }
    }

    // Mention anything weird in the data
    if nr_missing_guids > 0 {
        eprintln!("WARNING: Ignored {} RSS entres without a GUID", nr_missing_guids);
    }

    // We only rewrite the RSS in case there are additional entires
    //
    // Updates of any other field is not important
    if items_extra.len() == 0 {
       if opt.verbose {
           println!("No changes made");
        }
        return Ok(())
    }

    // Combine all entries into items_extra
    items_extra.append(&mut items_orig); // this clears items_orig

    // The number of entries is only limited in case entries are merged
    if opt.max_entries > 0 && items_extra.len() > opt.max_entries  {
        if opt.verbose {
            println!("Restricting RSS size to newest {} entries", opt.max_entries);
        }
        items_extra.truncate(opt.max_entries);
    }

    // Add the entries back to the RSS feed
    channel.set_items(items_extra);

    // And write the new file
    let mut outfile = tempfile_fast::Sponge::new_for("/home/olav/src/rss-combine/rss-out.xml").unwrap();
    channel.pretty_write_to(&mut outfile, b' ', 2).unwrap(); // // write to the channel to a writer
    outfile.commit().expect("Cannot store merged RSS back into main RSS file");

    Ok(())
}


fn main() {
    std::process::exit(match run_app() {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("error: {:?}", err);
            1
        }
    });
}
