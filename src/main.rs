#[macro_use]
extern crate lazy_static;
extern crate csv;
extern crate elapsed;
extern crate rayon;
extern crate regex;

use elapsed::measure_time;
use rayon::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;

struct Document {
    content: String,
}

impl Document {
    fn new(content: String) -> Self {
        lazy_static! {
            static ref remove_punc: Regex = Regex::new(r"[^0-9a-zA-Z ]+").unwrap();
        }

        Document {
            content: remove_punc.replace_all(&content, "").to_string(),
        }
    }
}

fn evaluate_bool_to_int(val: bool) -> u8 {
    match val {
        true => 1,
        false => 0,
    }
}

struct Filter {
    raw_string: String,
    dependencies: Vec<String>,
}

impl Filter {
    fn new(filter: &str) -> Self {
        lazy_static! {
            static ref dependency_finder: Regex = Regex::new(r"'([^']*)'").unwrap();
        }

        Filter {
            raw_string: filter.to_string(),
            dependencies: dependency_finder
                .captures_iter(filter)
                .map(|cap| cap[0].replace('\'', "").replace("‘", "'").to_string())
                .collect(),
        }
    }

    fn eval_document(&self, document: Document) -> bool {
        let mut hmap = HashMap::new();
        for dep in &self.dependencies {
            hmap.insert(
                dep.to_string(),
                match document.content.find(dep) {
                    Some(_) => true,
                    None => false,
                },
            );
        }

        let mut filter: String = self.raw_string.clone();

        for (term, present) in &hmap {
            filter = filter.replace(
                &format!("\'{}\'", term),
                &evaluate_bool_to_int(*present).to_string(),
            );
        }

        filter = filter.replace(" ", "");
        filter = filter.replace("OR", "|");
        filter = filter.replace("AND", "&");
        filter = filter.replace("NOT", "~");

        return eval_str(filter);
    }

    fn eval_document_set(&self, file_name: &str) -> Result<Vec<u8>, Box<Error>> {
        let mut evaluation = Vec::new();

        let mut rdr = csv::Reader::from_path(file_name).unwrap();
        for document in rdr.records() {
            let doc = document?;
            evaluation.push(evaluate_bool_to_int(
                self.eval_document(Document::new(doc[9].to_string())),
            ));
        }

        Ok(evaluation)
    }
}

fn eval_str(string: String) -> bool {
    let mut s = string.clone();

    lazy_static! {
        static ref single_val_finder_true: Regex = Regex::new(r"\(1\)").unwrap();
        static ref single_val_finder_false: Regex = Regex::new(r"\(0\)").unwrap();
        static ref question_mark_remover: Regex = Regex::new(r"\?").unwrap();
    }

    let mut count = 0;

    loop {
        if s.len() == 1 {
            match s.as_str() {
                "1" => return true,
                "0" => return false,
                &_ => {
                    println!("String Error: {:?}", s);
                    return false;
                }
            }
        }

        s = single_val_finder_true.replace_all(&s, "1").to_string();
        s = single_val_finder_false.replace_all(&s, "0").to_string();
        s = question_mark_remover.replace_all(&s, "").to_string();

        s = s.replace("\"", "");
        s = s.replace(".", "");
        s = s.replace("+", "");
        s = s.replace("-", "");

        s = s.replace("0|0", "0");
        s = s.replace("0|1", "1");
        s = s.replace("1|0", "1");
        s = s.replace("1|1", "1");

        s = s.replace("0&0", "0");
        s = s.replace("0&1", "0");
        s = s.replace("1&0", "0");
        s = s.replace("1&1", "1");

        s = s.replace("0~0", "0");
        s = s.replace("0~1", "0");
        s = s.replace("1~0", "1");
        s = s.replace("1~1", "0");

        count += 1;

        if count > 20 {
            s = s.replace("&(", "&");
            s = s.replace(")|", "&");
            s = s.replace("01", "1");
            s = s.replace("10", "1");
            s = s.replace("11", "1");
            s = s.replace("00", "0");
            s = s.replace("\"", "");
            s = s.replace(".", "");
            s = s.replace("+", "");
            s = s.replace("-", "");
            s = s.replace("(", "");
            s = s.replace(")", "");
            s = s.replace("|&", "&");
            s = s.replace("*", "");
            s = s.replace("||", "|");
            s = s.replace("0|", "0");
            s = s.replace("1|", "1");
            s = s.replace("&1", "1");
            s = s.replace("1&", "1");
            s = s.replace("&0", "0");
            s = s.replace("0&", "0");
            s = s.replace("/", "");
            s = s.replace("|1", "1");
            s = s.replace("|0", "0");
            s = s.replace(":", "");
        }

        if count > 60 {
            println!("Error: {}", s);
        }
    }
}

#[derive(Debug)]
struct Results {
    true_positives: Vec<u64>,
    false_positives: Vec<u64>,
    true_negatives: Vec<u64>,
    false_negatives: Vec<u64>,
    elapsed: Vec<u64>,
}

impl Results {
    fn new(results: Vec<Vec<u8>>, elapsed: Vec<u64>, relative_index: usize) -> Self {
        Results {
            true_positives: results
                .iter()
                .map(|res| -> u64 {
                    let mut count = 0;
                    for i in 0..res.len() {
                        if res[i] == 1 && results[relative_index][i] == 1 {
                            count += 1;
                        }
                    }
                    count
                })
                .collect(),
            false_positives: results
                .iter()
                .map(|res| -> u64 {
                    let mut count = 0;
                    for i in 0..res.len() {
                        if res[i] == 1 && results[relative_index][i] == 0 {
                            count += 1;
                        }
                    }
                    count
                })
                .collect(),
            true_negatives: results
                .iter()
                .map(|res| -> u64 {
                    let mut count = 0;
                    for i in 0..res.len() {
                        if res[i] == 0 && results[relative_index][i] == 0 {
                            count += 1;
                        }
                    }
                    count
                })
                .collect(),
            false_negatives: results
                .iter()
                .map(|res| -> u64 {
                    let mut count = 0;
                    for i in 0..res.len() {
                        if res[i] == 0 && results[relative_index][i] == 1 {
                            count += 1;
                        }
                    }
                    count
                })
                .collect(),
            elapsed,
        }
    }
}

#[derive(Clone)]
struct Experiment {
    query_set: Vec<String>,
}

fn run_experiments(
    path_to_src: &str,
    document_set: &str,
    number_of_queries: usize,
    relative_index: usize,
) -> Result<Vec<Results>, Box<Error>> {
    let mut rdr = csv::Reader::from_path(path_to_src).unwrap();
    let mut experiments = Vec::new();

    for exp in rdr.records() {
        let queries = exp?;
        experiments.push(Experiment {
            query_set: (0..number_of_queries)
                .map(|v| -> String { queries[v].to_string().replace("  ", " ") })
                .collect(),
        });
    }

    Ok(experiments
        .par_iter()
        .map(|exp| -> Results {
            let mut res = Vec::new();
            let mut elapsed = Vec::new();

            for query in exp.clone().query_set {
                let (elapsed_time, experiment_results) =
                    measure_time(|| Filter::new(&query).eval_document_set(document_set));

                res.push(experiment_results.unwrap());
                elapsed.push(elapsed_time.seconds());
            }

            Results::new(res, elapsed, relative_index)
        })
        .collect())
}

fn write_results(
    path: &str,
    results: Vec<Results>,
    query_labels: Vec<String>,
) -> Result<(), Box<Error>> {
    let mut wtr = csv::Writer::from_path(path)?;

    let mut header = Vec::new();
    let append_vals = vec![
        " True Positives",
        " False Positives",
        " True Negatives",
        " False Negatives",
        " Elapsed",
    ];

    query_labels.iter().for_each(|label| {
        append_vals
            .iter()
            .for_each(|val| header.push(label.to_string() + val))
    });
    wtr.write_record(&header)?;

    results
        .iter()
        .map(|res| -> Vec<String> {
            let mut row = Vec::new();
            for i in 0..res.elapsed.len() {
                row.push(res.true_positives[i].to_string());
                row.push(res.false_positives[i].to_string());
                row.push(res.true_negatives[i].to_string());
                row.push(res.false_negatives[i].to_string());
                row.push(res.elapsed[i].to_string());
            }
            row
        })
        .for_each(|v| wtr.write_record(&v).unwrap());

    wtr.flush()?;

    Ok(())
}

fn main() {
    let query_labels = vec![
        "Base".to_string(),
        "Human".to_string(),
        "Initial".to_string(),
        "Xu".to_string(),
    ];
    let results = run_experiments("queries.csv", "articles.csv", query_labels.len(), 1).unwrap();
    write_results("raw_data.csv", results, query_labels).expect("Error while writing results");
}
