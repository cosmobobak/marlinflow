# Marlinflow
### Neural Network training repository for the [Black Marlin](https://github.com/dsekercioglu/blackmarlin) chess engine.

This repo is or was relied upon by a number of other engines, including but not limited to [Viridithas](https://github.com/cosmobobak/viridithas), [Svart](https://github.com/crippa1337/svart), and [Carp](https://github.com/dede1751/carp).

# Requirements
- Python 3
- Cargo (Rust)
- Numpy
- PyTorch

# Usage
1. Clone the repo with `git clone https://github.com/dsekercioglu/marlinflow`
2. Build the binary parser, using native CPU optimisations for your system:

On Windows, do
```powershell
cd parse
$env:RUSTFLAGS="-C target-cpu=native"
cargo build --release
```
On Linux, do
```bash
cd parse
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

3. locate the resulting `.so`/`.dll` in the `target/release/` directory and move it to the `trainer/` directory, renamed as libparse.so/libparse.dll.
4. Create some directories for training output in the `trainer/` directory:

In `trainer/`, do
```bash
mkdir nn
mkdir runs
```

5. Decide upon the directory in which you want to store your training data. (simply making a `data/` directory inside `trainer/` is a solid option)
6. Place your binary data file in the directory created in step 5. (if you don't have one, consult [Getting Data](#getting-data))
7. In `trainer/`, run `main.py` with the proper command line arguments:

A typical invocation for training a network looks like this:
```bash
python main.py --data-root data --train-id net0001 --lr 0.001 --epochs 45 --lr-drop 30 --batch-size 16384 --wdl 0.3 --scale 400 --save-epochs 5
```

Here, `--data-root` is the directory created in step 5, `--train-id` is the name of the training run, `--lr` is the learning rate, `--epochs` is the number of epochs to train for, `--lr-drop` is the number of epochs after which the learning rate is dropped by a factor of 10, `--batch-size` is the batch size, `--wdl` is the weight of the WDL loss (1.0 would train the network to only predict game outcome, while 0.0 would aim to predict only eval, and other values interpolate between the two), `--scale` is the multiplier for the sigmoid output of the final neuron, and `--save-epochs n` tells the trainer to save the network every `n` epochs.

8. The trainer will output a number of files in the `nn/` directory - files of the form `net0001_X` are saved state_dict files, and `net0001.json` is a JSON file containing the final weights of the network. The trainer will also output a record of losses in the `runs/` directory. In order to use the network, you will need to convert the JSON file into a more usable format, and you will almost certainly want to quantise it. For simple perspective networks, this can be done with [nnue-jsontobin](https://github.com/cosmobobak/nnue-jsontobin), although for more complex networks like HalfKP and HalfKA you will need to employ some elbow grease.

# Getting Data
To convert a file in the marlinflow text format into a binary data file, use marlinflow-utils, which is build in much the same way as the parser:
```bash
cd utils
RUSTFLAGS="-C target-cpu=native" cargo build --release
```
The resulting binary will be in `target/release/`, and can be invoked as follows:
```bash
target/release/marlinflow-utils txt-to-data <INPUT.txt> --output <OUTPUT.bin>
```

# Text Format
Marlinflow accepts a specific text format for conversion into binary data files, with lines set out as following:
```
<fen0> | <eval0> | <wdl0>
<fen1> | <eval1> | <wdl1>
```
Here, `<fen>` is a [FEN string](https://www.chessprogramming.org/Forsyth-Edwards_Notation), `<eval>` is a evaluation in centipawns from white's point of view, and `<wdl>` is 1.0, 0.5, or 0.0, representing a win for white, a draw, or a win for black, respectively.

# Marlinflow-Utils
`marlinflow-utils` is a program that provides a number of utilities for working with marlinflow. These are as follows:
- `txt-to-data` converts a text file in the marlinflow text format into a binary data file.
- `shuffle` shuffles a binary data file. It is extremely important to shuffle your data before training, to prevent overfitting.
- `interleave` randomly interleaves binary data files. This allows you to cleanly combine data from multiple sources without requiring a re-shuffle.
- `stats` will scan a binary data file and output some possibly useful statistics about it.
- `count` will tell you how many positions are in a binary data file.
- `convert` will convert an NNUE JSON file into the BlackMarlin NNUE format. (currently only supports HalfKP)