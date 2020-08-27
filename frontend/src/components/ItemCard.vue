<template>
  <v-card
    class="mx-auto"
    color="grey lighten-4"
    max-width="600"
  >
    <v-card-title>
      <v-row align="start">
        <div class="caption grey--text text-uppercase">
          {{ name }} ({{ id }})
        </div>
        <div>
          <span
            class="display-2 font-weight-black"
            v-text="latestPrice"
          ></span>
        </div>
      </v-row>

      <v-spacer></v-spacer>

      <v-btn icon class="align-self-start" size="28" @click="$emit('show-item-detail', id)">
        <v-icon>mdi-arrow-right-thick</v-icon>
      </v-btn>
    </v-card-title>

    <v-sheet color="transparent">
      <v-sparkline
        :key="String(value)"
        :smooth="18"
        :gradient="['#f72047', '#ffd200', '#1feaea']"
        :line-width="3"
        :value="data"
        auto-draw
        stroke-linecap="round"
      ></v-sparkline>
    </v-sheet>
  <div :id="chartId" />
  </v-card>
</template>

<script lang="ts">
import Vue from "vue";
import * as d3 from "d3";

interface Price {
  ts: number;
  value: number;
}

interface Series {
  name: string;
  prices: Price[];
}

export default Vue.extend({
  name: "Chart",
  created() {
    const svg = d3
        .select("#chart_" + this.id)
        .append("svg")
        .attr("width", 1024)
        .attr("height", 768),
      margin = { top: 50, right: 20, bottom: 100, left: 100 },
      width = +svg.attr("width") - margin.left - margin.right,
      height = +svg.attr("height") - margin.top - margin.bottom,
      g = svg
        .append("g")
        .attr(
          "transform",
          "translate(" + margin.left + "," + margin.top + ")"
        );

    const xMin = d3.min(this.data.prices, d => {
      return d["ts"];
    });
    const xMax = d3.max(this.data.prices, d => {
      return d["ts"];
    });
    const yMin = d3.min(this.data.prices, d => {
      return d["value"];
    });
    const yMax = d3.max(this.data.prices, d => {
      return d["value"];
    });

    const focus = svg
      .append("g")
      .attr("class", "focus")
      .style("display", "none");
    focus.append("circle").attr("r", 4.5);
    focus.append("line").classed("x", true);
    focus.append("line").classed("y", true);

    const bisectDate = d3.bisector(d => d["ts"]).left;

    const xs = d3
      .scaleTime()
      .domain([new Date(xMin), new Date(xMax)])
      .range([0, width]);
    const ys = d3
      .scaleLinear()
      .domain([yMin - 5, yMax])
      .range([height, 0]);

    const onMouseOver = (e: SVGPathElement, p: Price[]) => {
      console.log(e, p);
      const x0 = xs.invert(d3.event.pageX);
      const i = bisectDate(this.data.prices, x0, 1);
      const d0 = this.data.prices[i - 1];
      const d1 = this.data.prices[i];
      console.log(x0, i, d0, d1, e);
      if (d1) {
        const d = x0 - d0.ts > d1.ts - x0 ? d1 : d0;
        focus.attr(
          "transform",
          `translate(${xs(new Date(d.ts))}, ${ys(new Date(d.ts))})`
        );
        focus
          .select("line.x")
          .attr("x1", 0)
          .attr("x2", -xs(d.ts))
          .attr("y1", 0)
          .attr("y2", 0);

        focus
          .select("line.y")
          .attr("x1", 0)
          .attr("x2", 0)
          .attr("y1", 0)
          .attr("y2", height - ys(d.value));

        focus.select("text").text(d.value);
        focus.append("g").text(d.value);
        svg
          .append("text")
          .attr("text-anchor", "end")
          .attr("style", "font: bold 20px sans-serif")
          .attr("transform", "rotate(-90)")
          .attr("x", -margin.top - 20)
          .attr("y", -margin.left + 90)
          .text(d.value);
      }
    };

    const line = d3
      .line()
      .x(function(d: Price, i) {
        return xs(d.ts);
      })
      .y(function(d: Price, i) {
        return ys(d.value);
      });

    g.append("g")
      .attr("transform", "translate(0," + height + ")")
      .call(
        d3.axisBottom(xs)
      )
      .selectAll("text")
      .attr("transform", "translate(-10,10)rotate(-45)")
      .style("text-anchor", "end")
      .style("font-size", 9)
      .style("fill", "#69a3b2")
      .select(".domain");

    g.append("g")
      .call(d3.axisLeft(ys))
      .append("text")
      .attr("fill", "#000")
      .attr("x", width / 2)
      .attr("y", 0)
      .attr("text-anchor", "end")
      .style("font-size", "20px")
      .style("font-weight", "bold")
      .text(this.data.name);

    g.append("path")
      .datum(this.data.prices)
      .attr("fill", "none")
      .attr("stroke", "steelblue")
      .attr("stroke-linejoin", "round")
      .attr("stroke-linecap", "round")
      .attr("stroke-width", 1.5)
      .attr("d", line);

    svg
      .on("mouseover", () => focus.style("display", null))
      .on("mouseout", () => focus.style("display", "none"))
      .on("mousemove", onMouseOver);
  },
  
  data: function () {
    return {
      latestPrice: this.data[this.data.length - 1].value,
      chartId: this.id
    }
  },

  props: {
    data: Array,
    name: String,
    id: Number,
  }
});
</script>
