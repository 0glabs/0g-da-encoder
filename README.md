# 0g Data Avalability Encoder

This project is a 0g data availability module that handles mathematical computations, including encoding logic, verification logic, and public parameters for the encoding cryptographic protocol.

## Features

- **parallel**: Uses parallel algorithms for computations, maximizing CPU resource utilization.
- **cuda**: Uses GPU for computations, applicable only on platforms with NVIDIA GPUs.

Note: GPU support is currently tested with NVIDIA 12.04 drivers on the RTX 4090. Other NVIDIA GPUs may require parameter adjustments and have not been tuned yet.

## Preparation

### Install Rust

Ensure you have `curl` installed.

Run the following command to install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, add the cargo bin directory to your `PATH` environment variable:
```bash
source $HOME/.cargo/env
```

Verify the installation:
```bash
rustc --version
```

### Install other dependencies

```bash
# Install Protocol Buffers Compiler
sudo apt-get install -y protobuf-compiler

# Install a specific nightly Rust toolchain and rustfmt
rustup toolchain install nightly-2024-02-04-x86_64-unknown-linux-gnu
rustup component add --toolchain nightly-2024-02-04-x86_64-unknown-linux-gnu rustfmt

# Add the necessary Rust target
rustup target add x86_64-unknown-linux-gnu
```

### Install CUDA (for GPU feature)
Ensure you have an NVIDIA GPU with the required drivers. Then following the instruction from [CUDA Toolkit](https://developer.nvidia.com/cuda-12-4-1-download-archive).

Verify the installation:
```bash
nvidia-smi
nvcc --version
```

## Building Public Parameters

The public parameters for the cryptographic protocol are built in two steps:

### Download and process the perpetual power of tau
The [perpetual power of tau project](https://github.com/privacy-scaling-explorations/perpetualpowersoftau) is a cryptographic ceremony launched by Zcash, aimed at reducing the trust risk in trusted setups. The generated parameters from this ceremony are secure unless all participants collude.

We use the `challenge_0084` file from the [nearly most recent submission](https://github.com/privacy-scaling-explorations/perpetualpowersoftau/blob/master/0084_jpwang_response/README.md). 
```bash
curl https://pse-trusted-setup-ppot.s3.eu-central-1.amazonaws.com/challenge_0084 -o challenge_0084
```

### Build the AMT parameters
  
The DA encoder uses the AMT protocol described in the paper [LVMT](https://www.usenix.org/conference/osdi23/presentation/li-chenxing). You can either construct these parameters yourself or download pre-built files. The parameters will be placed in `./params`. The building process is deterministic, and the sha2sum of generated files is list as follows.

```
a132ba9fa48c338c478a3e9d7d1cde13d77c6096d3cca1ac28f091315ca58428  amt-prove-coset0-5DWgDV-10-20.bin
6c1d7837e5380ca7e09e1f396b4f8ff3ec546cabcccc7bc65f6439a75a791a80  amt-prove-coset0-mont-5DWgDV-10-20.bin
58ee8752e93408b79f3640d7712f5915213e69b872a3393ac67824d29d95379c  amt-prove-coset1-5DWgDV-10-20.bin
a9f4f6b07a0d66620d652227233c42d12d2726c00f802eadd0e46db68917885a  amt-prove-coset1-mont-5DWgDV-10-20.bin
0f12211c21816a55ef856fc4f22532e4a5f286bb68d7dfec0f8102a224613533  amt-prove-coset2-5DWgDV-10-20.bin
0314657436c124f2b00c7bb4e239dc551ab4f0f732516ad9dd656ec12b091c17  amt-prove-coset2-mont-5DWgDV-10-20.bin
18bb6b7ba10785a79810180ddd27a6d467d2c0e24e6335e5bc95998e02c6a4f6  amt-verify-coset0-5DWgDV-10-20.bin
19b024fed13e0ba60b17184c998dcccf12119b4fd0ab7c46394b8e024e99c48a  amt-verify-coset1-5DWgDV-10-20.bin
5660a89402df7d47885b304b566d92e1c42349744e202d45fc61a0893bd796c9  amt-verify-coset2-5DWgDV-10-20.bin
bc148cd9ce28f65a4bacbe174a232bee426afd32dfce73004ee38ca9afa46059  power-tau-5DWgDV-20.bin
```

**Choice 1: Download the pre-built files.** Please make sure you have `curl` installed, then run the command in the project root directory:
```sh
./dev_support/download_params.sh
```

**Choice 2: Construct the parameters yourself.** Run the following command in the project root directory:
```sh
./dev_support/build_params.sh challenge_0084
```

## Running the Server

Run the server with the following command:
```sh
cargo run -r -p server --features grpc/parallel,grpc/cuda -- --config run/config.toml
```
If you do not have a CUDA environment, remove the `cuda` feature.

DA Encoder will serve on port `34000` with specified [grpc interface](grpc/proto/encoder.proto). 


## Using the Verification Logic

Add the following to `Cargo.toml` of your crate:
```toml
zg-encoder = { git = "https://github.com/0glabs/0g-da-encoder.git" }
```
Use the `zg_encoder::EncodedSlice::verify` function for verifing. You can enable the `parallel` feature as you need. The `cuda` feature is not avaliable for the verification process.

## Benchmark the Performance

Run the following task
```bash
cargo bench -p grpc --features grpc/parallel,grpc/cuda --bench process_data --features zg-encoder/production_mode -- --nocapture
```

For the first run, it may take some time to build the public params (about 30 minutes on aws g5.4xlarge). Alternatively, you can place the downloaded public params `.bin` file in the `crates/amt/pp` directory.


## Development and Testing

Run the following script for complete testing:
```sh
./dev_support/test.sh
```
