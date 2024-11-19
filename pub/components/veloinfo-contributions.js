export class VeloinfoContributions extends LitElement {
    static properties = {
        name: {
            contributions: {}
        },
    };
    // Define scoped styles right with your component, in plain CSS
    static styles = css`
      :host {
        color: blue;
      }
    `;

    constructor() {
        super();
        // Declare reactive properties
        this.contributions = [];
    }

    // Render the UI as a function of component state
    render() {
        return html`
            <div class="flex cursor-pointer mb-2 mt-1" hx-get="/segment_panel/id/{{score_id}}" hx-target="#info">
                {{ score_circle }}
                <div class="content-start w-full">
                    <div class="flex flex-row justify-between">
                        <div class="flex">
                        <div class="text-xs"> {{ created_at}} </div>
                        <div class="text-xs ml-1"> ( {{ user_name }} ) </div>
                    </div>
               <div class="text-xs mr-2"> {{ timeago }} </div>
                    </div>
                <div class="font-bold text-sm">{{ name }}</div>
                <div class="flex flex-row">
                    {% match photo_path_thumbnail %}
                    {% when Some with (photo_path_thumbnail) %}
                    <img class="w-8 h-8 mx-2 rounded-sm" src="{{ photo_path_thumbnail }}" alt="photo">
                    {% when None %}
                    {% endmatch %}
                    <div class="text-sm text-gray-600">{{ comment }}</div>
                </div>
                </div>
            </div>
            <hr>`;
    }
}
customElements.define('veloinfo-contributions', VeloinfoContributions);
