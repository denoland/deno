<template>
  <!-- Basic double brace expressions -->
  <div class="basic" :style="{ width: a + b + 'px', height: '1px' }">
    {{a+b}}
  </div>

  <!-- Complex expressions with extra whitespace -->
  <div
    :class="{ visible:isVisible, large:width>50 }"
    :style="{
      backgroundColor:color,
      width:width+'px',
      height:width/2+'px',
      opacity:isVisible?1:0.5,
      transform: `translate(${a*2}px, ${b-10}px)`,
      borderRadius:Math.min(width, height)/4+'px'
    }"
  >
    <!-- Nested expressions -->
    <span :style="{ fontSize:12+(width/10)+'px' }">
      Content: {{items.length}} items
    </span>

    <!-- Complex object access -->
    <p>User: {{user.name}} ({{user.age}} years old)</p>

    <!-- Array methods -->
    <ul>
      <li
        v-for="item in items.filter(item => item > 1)"
        :key="item"
        :style="{ padding:item*2+'px' }"
      >
        {{item}}
      </li>
    </ul>

    <!-- Conditional expressions -->
    <div v-if="width > 75" :style="{ margin:isVisible?10:5+'px' }">
      Large content
    </div>
    <div v-else :style="{ margin:width/10+'px' }">
      Small content
    </div>
  </div>

  <!-- Attribute expressions -->
  <input
    type="text"
    :placeholder="user.name||'Enter name'"
    :value="a+b"
    :style="{ width:Math.max(100, width)+'px' }"
  />

  <!-- Mixed single brace and double brace -->
  <div :class="{ active:active }">
    Mixed: {{color}} - {{ width }}px
  </div>

  <!-- Template literals in attributes -->
  <div
    :title="`Size: ${width}x${height}`"
    :data-info="`User: ${user.name}, Items: ${items.length}`"
  >
    Template data
  </div>
</template>

<script lang="ts">
export default {
  data() {
    return {
      a:0 as number,
      b:0 as number,
      color:"red",
      width:100,
      height:50,
      isVisible:true,
      active:false,
      items:[1,2,3],
      user:{name:"John",age:30}
    };
  }
};
</script>

<style scoped>
.basic{
  border:1px solid black;
}
.visible{
  display:block;
}
.large{
  font-size:1.2em;
}
.active{
  background-color:#f0f0f0;
}
</style>
