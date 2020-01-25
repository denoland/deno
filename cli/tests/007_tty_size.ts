const { columns, rows } = Deno.ttySize();

if(!(typeof columns === 'number') || !(typeof rows === 'number')) {
    console.log('fail: expected numeric values');
} else {
    console.log('ok');
}