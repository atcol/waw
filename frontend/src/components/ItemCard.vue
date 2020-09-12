<template>

  <v-card
    class="mt-4 mx-auto"
    max-width="400"
  >
    

    <v-card-text class="pt-0">
      <div class="title font-weight-medium mb-2">{{ name }} ({{ id }})</div>
      <v-divider class="my-4"></v-divider>
      <v-list dense>
          <v-list-item>
            <v-list-item-content >Price:</v-list-item-content>
            <v-list-item-content class="align-end" ><v-chip dense color="blue" outlined>{{ data[0].value }}</v-chip></v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content >Timestamp:</v-list-item-content>
            <v-list-item-content class="align-end" >{{ new Date(data[0].ts * 1000).toISOString() }}</v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content >High:</v-list-item-content>
            <v-list-item-content class="align-end" >{{ max[1] }}</v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content >Low:</v-list-item-content>
            <v-list-item-content class="align-end" >{{ min[1] }}</v-list-item-content>
          </v-list-item>
        </v-list>
      <v-sheet
        class="v-sheet--offset mx-auto"
        color="white"
        elevation="2"
        max-width="calc(100% - 32px)"
      >
        <v-sparkline
          :key="data.value"
          :smooth="18"
          :gradient="['#f72047', '#ffd200', '#1feaea']"
          :line-width="3"
          :value="data"
          auto-draw
          stroke-linecap="round"
        ></v-sparkline>
      </v-sheet>
      <span class="caption grey--text font-weight-light" @mouseover="hover = true" @mouseleave="hover = false">
        <v-icon>mdi-arrow-down</v-icon>{{ min[1] }}
        <span v-if="hover">{{ new Date(max[0] * 1000).toISOString() }}</span>
      </span>
      <v-divider vertical></v-divider>
      <span class="caption grey--text font-weight-light" @mouseover="hover = true" @mouseleave="hover = false">
        <v-icon>mdi-arrow-up</v-icon>{{ max[1] }}
        <span v-if="hover">{{ new Date(max[0] * 1000).toISOString() }}</span>
      </span>
    </v-card-text>
    <div :id="`chart_${id}`" />
  </v-card>

</template>

<script lang="ts">
import Vue from "vue";
import * as d3 from "d3";

interface Price {
  ts: number;
  value: number;
}

export default Vue.extend({
  name: "item-card",
  data: function () {
    return {
      chartId: this.id,
      latestPrice: this.data[this.data.length - 1].value,
      hover: false,
    }
  },

  props: {
    data: Array,
    name: String,
    id: Number,
    min: Array,
    max: Array,
  }
});
</script>
