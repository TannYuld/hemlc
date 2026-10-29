async function loadLate() {
    await new Promise(resolve => setTimeout(resolve, 3000));
    return true;
}