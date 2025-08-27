#!/bin/bash

# Niva Dashboard - Build and Run Script for Raspberry Pi
# This script helps set up the development environment and run the dashboard

echo "Niva Dashboard - Raspberry Pi Setup"
echo "===================================="

# Function to install dependencies on Raspberry Pi
install_dependencies() {
    echo "Installing SDL2 and OpenGL ES development libraries..."
    sudo apt-get update
    sudo apt-get install -y \
        libsdl2-dev \
        libgles2-mesa-dev \
        libegl1-mesa-dev \
        build-essential \
        pkg-config
    
    echo "Dependencies installed successfully!"
}

# Function to build the project
build_project() {
    echo "Building Niva Dashboard..."
    cargo build --release
    
    if [ $? -eq 0 ]; then
        echo "Build successful!"
    else
        echo "Build failed!"
        exit 1
    fi
}

# Function to run the project
run_project() {
    echo "Running Niva Dashboard..."
    cargo run --release
}

# Main menu
case "$1" in
    "install")
        install_dependencies
        ;;
    "build")
        build_project
        ;;
    "run")
        run_project
        ;;
    "setup")
        install_dependencies
        build_project
        ;;
    *)
        echo "Usage: $0 {install|build|run|setup}"
        echo ""
        echo "Commands:"
        echo "  install  - Install system dependencies"
        echo "  build    - Build the project"
        echo "  run      - Run the project"
        echo "  setup    - Install dependencies and build"
        echo ""
        echo "Example: ./run.sh setup"
        exit 1
        ;;
esac
