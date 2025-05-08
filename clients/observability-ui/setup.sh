#!/bin/bash

# ICN Observability Dashboard Setup Script

echo "==============================================="
echo "  ICN Observability Dashboard Setup"
echo "==============================================="
echo ""

# Check if npm is installed
if ! command -v npm &> /dev/null; then
    echo "❌ Error: npm is not installed!"
    echo "Please install Node.js and npm before continuing."
    exit 1
fi

echo "✅ npm is installed"

# Install UI dependencies
echo "📦 Installing UI dependencies..."
npm install
if [ $? -ne 0 ]; then
    echo "❌ Error installing UI dependencies"
    exit 1
fi
echo "✅ UI dependencies installed"

# Create API server directory if not exists
if [ ! -d "server" ]; then
    mkdir -p server
fi

# Copy API files to server directory if they're not in the right location
if [ -f "src/server/api.js" ] && [ ! -f "server/api.js" ]; then
    echo "📂 Setting up API server..."
    cp src/server/api.js server/api.js
    cp src/server/package.json server/package.json
fi

# Install API dependencies
echo "📦 Installing API server dependencies..."
cd server
npm install
if [ $? -ne 0 ]; then
    echo "❌ Error installing API server dependencies"
    exit 1
fi
cd ..
echo "✅ API server dependencies installed"

# Create .env file if it doesn't exist
if [ ! -f ".env" ]; then
    echo "📝 Creating .env file..."
    echo "REACT_APP_API_URL=http://localhost:3001/api" > .env
    echo "PORT=3000" >> .env
    echo "✅ Created .env file"
else
    echo "ℹ️  .env file already exists"
fi

echo ""
echo "==============================================="
echo "  Setup Complete! 🎉"
echo "==============================================="
echo ""
echo "To start the dashboard:"
echo ""
echo "1. Start the API server:"
echo "   cd server && npm start"
echo ""
echo "2. In a new terminal, start the UI:"
echo "   npm start"
echo ""
echo "The dashboard will be available at http://localhost:3000"
echo "The API server will be running at http://localhost:3001"
echo ""
echo "Note: Make sure the ICN CLI is installed and in your PATH"
echo "==============================================="

# Make the script executable
chmod +x setup.sh 