#!/bin/bash
set -e

# Check for clean workspace
if [[ -n $(git status -s) ]]; then
    echo "Error: Working directory is not clean. Please commit or stash changes first."
    exit 1
fi

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | cut -d '"' -f 2)
echo "Current version: $CURRENT_VERSION"

# Request new version
read -p "Enter new version: " NEW_VERSION

# Validate semver format
if ! [[ $NEW_VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in format X.Y.Z"
    exit 1
fi

# Update Cargo.toml version
sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml

# Generate changelog
git cliff --tag $NEW_VERSION > CHANGELOG.md

# Stage changes
git add Cargo.toml CHANGELOG.md

# Commit changes
git commit -m "chore: release version $NEW_VERSION"

# Create tag
git tag -a "v$NEW_VERSION" -m "Release version $NEW_VERSION"

# Push changes and tags
echo "Pushing changes and tags..."
git push && git push --tags

echo "Release $NEW_VERSION completed successfully!"