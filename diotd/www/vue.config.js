const path = require('path');
let HtmlWebpackPlugin = require('html-webpack-plugin');
let HtmlWebpackInlineSourcePlugin = require('html-webpack-inline-source-plugin');

module.exports = {
    outputDir: process.env.OUT_DIR ? path.resolve(process.env.OUT_DIR, "www-dist") : path.resolve(__dirname, "dist"),
    configureWebpack: {
        plugins: [
            new HtmlWebpackPlugin({
                template: 'public/index.html',  //template file to embed the source
                inlineSource: '.(js|css)$' // embed all javascript and css inline
            }),
            new HtmlWebpackInlineSourcePlugin(HtmlWebpackPlugin)
        ]
    }
}