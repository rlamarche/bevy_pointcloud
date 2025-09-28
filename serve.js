const express = require('express');

const app = express();

// Add required headers to enable SharedArrayBuffer feature
app.use((req, res, next) => {
    res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
    res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
    next();
});

// Serve project dir
app.use(express.static(__dirname));

app.listen(8080, () => {
    console.log('Server started on http://localhost:8080');
});
