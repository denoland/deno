// Helper function to read user input from the terminal
async function getInput(promptText: string): Promise<string> {
    const buffer = new Uint8Array(1024); // Buffer to store input
    await Deno.stdout.write(new TextEncoder().encode(promptText)); // Show prompt text
    const n = <number>await Deno.stdin.read(buffer); // Read input from user
    return new TextDecoder().decode(buffer.subarray(0, n)).trim(); // Return trimmed string input
  }
  
  // Basic arithmetic operations
  function add(a: number, b: number): number {
    return a + b;
  }
  
  function subtract(a: number, b: number): number {
    return a - b;
  }
  
  function multiply(a: number, b: number): number {
    return a * b;
  }
  
  function divide(a: number, b: number): number {
    if (b === 0) throw new Error("Cannot divide by zero!");
    return a / b;
  }
  
  // Advanced operations
  function squareRoot(a: number): number {
    if (a < 0) throw new Error("Cannot take square root of negative number!");
    return Math.sqrt(a);
  }
  
  function power(a: number, b: number): number {
    return Math.pow(a, b);
  }
  
  // Trigonometric operations
  function sine(a: number): number {
    return Math.sin(a);
  }
  
  function cosine(a: number): number {
    return Math.cos(a);
  }
  
  function tangent(a: number): number {
    return Math.tan(a);
  }
  
  // Main calculator function
  async function calculator() {
    // Ask the user for the operation
    const operation = await getInput("Enter operation (+, -, *, /, sqrt, pow, sin, cos, tan): ");
  
    let result: number | undefined;
  
    try {
      if (operation === 'sqrt') {
        // Handle square root
        const num = parseFloat(await getInput("Enter number: "));
        result = squareRoot(num);
      } else if (operation === 'sin' || operation === 'cos' || operation === 'tan') {
        // Handle trigonometric functions
        const num = parseFloat(await getInput("Enter number (in radians): "));
        if (operation === 'sin') result = sine(num);
        if (operation === 'cos') result = cosine(num);
        if (operation === 'tan') result = tangent(num);
      } else if (operation === 'pow') {
        // Handle power function
        const base = parseFloat(await getInput("Enter base: "));
        const exponent = parseFloat(await getInput("Enter exponent: "));
        result = power(base, exponent);
      } else {
        // Handle basic arithmetic (+, -, *, /)
        const num1 = parseFloat(await getInput("Enter first number: "));
        const num2 = parseFloat(await getInput("Enter second number: "));
        if (operation === '+') result = add(num1, num2);
        if (operation === '-') result = subtract(num1, num2);
        if (operation === '*') result = multiply(num1, num2);
        if (operation === '/') result = divide(num1, num2);
      }
  
      // Output the result
      console.log(Result: ${result});
    } catch (error) {
      console.error("Error: " + error.message);
    }
  }
  
  // Start the calculator
  calculator();  
