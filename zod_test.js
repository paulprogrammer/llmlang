const { z } = require('zod');

const schema = z.object({
  type: z.literal('object')
});

try {
  schema.parse({ type: 'object' });
  console.log('Passed string "object"');
} catch (e) {
  console.log('Failed string "object":', JSON.stringify(e.errors, null, 2));
}

try {
  schema.parse({ type: 'Object' });
} catch (e) {
  console.log('Failed string "Object":', JSON.stringify(e.errors, null, 2));
}

const enumSchema = z.object({
  type: z.enum(['object'])
});

try {
  enumSchema.parse({ type: 'Object' });
} catch (e) {
  console.log('Failed enum "Object":', JSON.stringify(e.errors, null, 2));
}
