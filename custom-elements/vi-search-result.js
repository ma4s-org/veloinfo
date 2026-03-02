export default class ViSearchResult extends HTMLElement {
    constructor(results, query) {
        super();
        this.results = results;
        this.query = query;
    }

    connectedCallback() {
        this.render();
    }

    render() {
        this.innerHTML = `
            <div style="font-size: 1rem;">
                recherche de <span style="font-weight: 700;">${this.query}</span>
            </div>
            ${this.results.map(result => `
                <search-result name="${result.name}" lng="${result.lng}" lat="${result.lat}"></search-result>
            `).join('')}
        `;
    }
}

customElements.define('vi-search-result', ViSearchResult);