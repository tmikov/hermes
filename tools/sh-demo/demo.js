function createCounterWithGenerator() {
    let count = 0; // Shared mutable variable

    const increment = (by = 1) => count += by;

    function* doSteps(steps) {
        for (let i = 0; i < steps; i++)
            yield increment(); // Use the default parameter for `by` in `increment`
    }

    return {
        increment,
        doSteps
    };
}

console.log("Closures\n=========");
const counter = createCounterWithGenerator();
const steps = 5;

console.log(`Generating ${steps} increments with default step size:`);
for (const value of counter.doSteps(steps))
    console.log(value);

console.log("Further increments:");
for (const value of counter.doSteps(3))
    console.log(value);

// ===================================================================

function show_tdz() {
    function getval() {
        return val;
    }
    let val = getval() + 1;
}

console.log("\nTDZ\n=========");
try {
    show_tdz();
} catch (e) {
    console.log("TDZ Error!", e.stack);
}

// ===================================================================

const prototypeObj = {
    first: "I am in the prototype"
};

const obj = {
    get second() {
        // Add and increment the `third` property
        if (!this.third) {
            this.third = 1; // Initialize if it doesn't exist
        } else {
            this.third++;
        }
        return `Getter executed, third is now ${this.third}`;
    },

    __proto__: prototypeObj // Set prototype using object literal syntax
};

console.log("\nPrototypical Inheritance\n=========");

console.log("First property:", obj.first); // Inherited from prototype
console.log("Second property:", obj.second); // Triggers the getter
console.log("Third property:", obj.third); // Dynamically added and incremented
console.log("Second property again:", obj.second); // Getter increments third
console.log("Third property now:", obj.third); // Reflects incremented value

// ===================================================================

class PrototypeClass {
    constructor() {}

    // Define `first` as a getter in the prototype
    get first() {
        return "I am in the prototype";
    }
}

class DerivedClass extends PrototypeClass {
    constructor() {
        super();
    }

    get second() {
        // Add and increment the `third` property
        if (!this.third) {
            this.third = 1; // Initialize if it doesn't exist
        } else {
            this.third++;
        }
        return `Getter executed, third is now ${this.third}`;
    }
}

console.log("\nClasses\n=========");

const clInst = new DerivedClass();

console.log("First property:", clInst.first); // Inherited from PrototypeClass
console.log("Second property:", clInst.second); // Triggers the getter
console.log("Third property:", clInst.third); // Dynamically added and incremented
console.log("Second property again:", clInst.second); // Getter increments third
console.log("Third property now:", clInst.third); // Reflects incremented value
