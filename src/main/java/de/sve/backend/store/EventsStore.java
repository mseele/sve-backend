package de.sve.backend.store;

import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;
import java.util.stream.Collectors;

import com.google.api.core.ApiFuture;
import com.google.cloud.firestore.CollectionReference;
import com.google.cloud.firestore.DocumentSnapshot;
import com.google.cloud.firestore.Firestore;
import com.google.cloud.firestore.FirestoreOptions;
import com.google.cloud.firestore.QueryDocumentSnapshot;
import com.google.cloud.firestore.QuerySnapshot;

import de.sve.backend.model.events.Event;
import de.sve.backend.model.events.EventType;

public class EventsStore implements AutoCloseable {

	private Firestore fService;

	protected EventsStore() {
		this.fService = FirestoreOptions.getDefaultInstance().getService();
	}

	protected List<Event> loadEvents() throws InterruptedException, ExecutionException {
		ApiFuture<QuerySnapshot> future = collection().get();
		QuerySnapshot querySnapshot = future.get();
		List<Event> events = new ArrayList<>();
		for (QueryDocumentSnapshot document : querySnapshot.getDocuments()) {
			events.add(create(document));
		}
		return events;
	}

	protected Event loadEvent(String id) throws InterruptedException, ExecutionException {
		ApiFuture<DocumentSnapshot> future = collection().document(id).get();
		DocumentSnapshot document = future.get();
		if (document.exists()) {
			return create(document);
		}
		return null;
	}

	protected void saveEvent(Event event) throws InterruptedException, ExecutionException {
		List<String> dates = event.dates()
								  .stream()
								  .map(LocalDateTime::toString)
								  .collect(Collectors.toList());
		Map<String, Object> data = new HashMap<>();
		data.put("sheetId", event.sheetId()); //$NON-NLS-1$
		data.put("gid", event.gid()); //$NON-NLS-1$
		data.put("type", event.type().name()); //$NON-NLS-1$
		data.put("name",event.name()); //$NON-NLS-1$
		data.put("sortIndex", event.sortIndex()); //$NON-NLS-1$
		data.put("visible", event.visible()); //$NON-NLS-1$
		data.put("beta", event.beta()); //$NON-NLS-1$
		data.put("shortDescription", event.shortDescription()); //$NON-NLS-1$
		data.put("description", event.description()); //$NON-NLS-1$
		data.put("image", event.image()); //$NON-NLS-1$
		data.put("light", event.light()); //$NON-NLS-1$
		data.put("dates", dates); //$NON-NLS-1$
		data.put("customDate", event.customDate()); //$NON-NLS-1$
		data.put("durationInMinutes", event.durationInMinutes()); //$NON-NLS-1$
		data.put("maxSubscribers", event.maxSubscribers()); //$NON-NLS-1$
		data.put("subscribers", event.subscribers()); //$NON-NLS-1$
		data.put("costMember", event.costMember()); //$NON-NLS-1$
		data.put("costNonMember", event.costNonMember()); //$NON-NLS-1$
		data.put("waitingList", event.waitingList()); //$NON-NLS-1$
		data.put("maxWaitingList", event.maxWaitingList()); //$NON-NLS-1$
		data.put("location", event.location()); //$NON-NLS-1$
		data.put("bookingTemplate", event.bookingTemplate()); //$NON-NLS-1$
		data.put("waitingTemplate", event.waitingTemplate()); //$NON-NLS-1$
		data.put("externalOperator", event.externalOperator()); //$NON-NLS-1$
		collection().document(event.id()).set(data).get();
	}

	protected void deleteEvent(Event event) throws InterruptedException, ExecutionException {
		collection().document(event.id()).delete().get();
	}

	private CollectionReference collection() {
		return this.fService.collection("events"); //$NON-NLS-1$
	}

	@Override
	public void close() throws Exception {
		this.fService.close();
	}

	private static final Event create(DocumentSnapshot document) {
		@SuppressWarnings("unchecked")
		List<String> dates = (List<String>) document.get("dates"); //$NON-NLS-1$
		return Event.create(document.getId(),
							document.getString("sheetId"), //$NON-NLS-1$
							document.getLong("gid"), //$NON-NLS-1$
							EventType.valueOf(document.getString("type")), //$NON-NLS-1$
							document.getString("name"), //$NON-NLS-1$
							document.getLong("sortIndex"), //$NON-NLS-1$
							document.getBoolean("visible"), //$NON-NLS-1$
							document.getBoolean("beta"), //$NON-NLS-1$
							document.getString("shortDescription"), //$NON-NLS-1$
							document.getString("description"), //$NON-NLS-1$
							document.getString("image"), //$NON-NLS-1$
							document.getBoolean("light"), //$NON-NLS-1$
							dates.stream().map(LocalDateTime::parse).collect(Collectors.toList()),
							document.getString("customDate"), //$NON-NLS-1$
							document.getLong("durationInMinutes"), //$NON-NLS-1$
							document.getLong("maxSubscribers"), //$NON-NLS-1$
							document.getLong("subscribers"), //$NON-NLS-1$
							document.getDouble("costMember"), //$NON-NLS-1$
							document.getDouble("costNonMember"), //$NON-NLS-1$
							document.getLong("waitingList"), //$NON-NLS-1$
							document.getLong("maxWaitingList"), //$NON-NLS-1$
							document.getString("location"), //$NON-NLS-1$
							document.getString("bookingTemplate"), //$NON-NLS-1$
							document.getString("waitingTemplate"), //$NON-NLS-1$
							document.getBoolean("externalOperator")); //$NON-NLS-1$
	}

}
