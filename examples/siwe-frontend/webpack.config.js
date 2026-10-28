const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = {
  mode: 'development',
  entry: './src/index.js',
  resolve: {
    fallback: {
      net: false,
      tls: false,
      fs: false,
    }
  },
  output: {
    filename: 'main.js',
    path: path.resolve(__dirname, 'dist')
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: './src/index.html',
      filename: 'index.html'
    }),
  ],
  devServer: {
    proxy: [
      {
        context: ['/api'],
        target: 'http://127.0.0.1:1234',
        changeOrigin: true,
        pathRewrite: { '^/api': '' }
      }
    ]
  }
};

