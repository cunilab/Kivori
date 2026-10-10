// Turns an imported `.md` file into its text, so curated release notes can be `import`ed at build time.
module.exports = function mdLoader(source) {
  return `export default ${JSON.stringify(String(source))};`;
};
