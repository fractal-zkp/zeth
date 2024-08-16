# Zeth

**Zeth** is a Reth-based sequencer node for the Polygon Type 1 zkEVM. It facilitates witness generation that can be used as input to the Polygon prover. The project supports both SQLite and PostgreSQL backends for witness persistence.

## Features

- **Reth-Based Sequencer Node**: Operates as a sequencer node for the Type 1 zkEVM.
- **Witness Generation**: Supports witness generation for the Type 1 prover.
- **Backend Support**: Offers both SQLite and PostgreSQL backends for witness data persistence.

## Installation

To install Zeth, follow these steps:

1. **Clone the Repository:**

   ```bash
   git clone https://github.com/fractal-zkp/zeth.git
   cd zeth
   ```

2. **Build the Project:**

   Use Cargo to build the project in release mode:

   ```bash
   cargo build --release
   ```

## Usage

Once the project is built, you can run a development node with the following command:

```bash
./target/release/zeth node --dev --dev.block-max-transactions 1
```

This command runs a development node with a maximum of one transaction per block.

## Contributing

Contributions are welcome! Please fork the repository and create a pull request with your changes. Make sure to follow the established coding standards and include relevant tests.

## License

This project is licensed under the MIT License. See the `LICENSE` file for more details.

## Contact Information

If you have any questions or need further assistance, feel free to create an issue in the repository.
